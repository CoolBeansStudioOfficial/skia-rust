// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkRuntimeEffectTest.cpp (chrome/m156)

//! `SkRuntimeEffectTest`: runtime effect factories and shaders.
//!
//! Ported here: the factory and reflection tests, the shader, color filter and blender tests and
//! the builders. Not ported yet, and left `todo` in the manifest with the reason:
//! - `SkRuntimeShaderSampleCoords` (it needs `GrSkSLFP`, Ganesh) and the Ganesh variants.

// The ported tests keep the C++ declaration order and function lengths.
#![allow(
    clippy::too_many_lines,
    clippy::cast_possible_wrap,
    clippy::items_after_statements
)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::capabilities::Capabilities;
use skia_rust_core::color::{Color, Color4f, colors};
use skia_rust_core::color_filters;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::runtime_effect::{
    ChildPtr, Options, RuntimeEffect, RuntimeShaderBuilder, uniform as uniform_flags,
};
use skia_rust_core::runtime_effect_priv;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::shaders::image_shader::ImageShader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::blenders as effects_blenders;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::image_factories::texture_from_image;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::tools::test_surface::{GraphiteTestSurface, TestSurface};
use skia_rust_sksl::program_settings::Version;

use crate::{Reporter, def_graphite_adapter_test, def_test, errorf, reporter_assert};
use skia_rust_core::image::RequiredProperties;

// Port of: tests/SkRuntimeEffectTest.cpp#L89-L96 (chrome/m156)
fn test_invalid_effect(r: &mut Reporter, src: &str, expected: &str) {
    let result = RuntimeEffect::make_for_shader(src, None);
    reporter_assert!(r, result.is_err());
    let error_text = result.err().unwrap_or_default();
    reporter_assert!(
        r,
        error_text.contains(expected),
        "Expected error message to contain \"{}\". Actual message: \"{}\"",
        expected,
        error_text
    );
}

const EMPTY_MAIN: &str = "half4 main(float2 p) { return half4(0); }";

// Port of: tests/SkRuntimeEffectTest.cpp#L100-L106 (chrome/m156)
def_test!(SkRuntimeEffectInvalid_NoInVariables, |r| {
    // 'in' variables aren't allowed at all:
    test_invalid_effect(r, &format!("in bool b;{EMPTY_MAIN}"), "'in'");
    test_invalid_effect(r, &format!("in float f;{EMPTY_MAIN}"), "'in'");
    test_invalid_effect(r, &format!("in float2 v;{EMPTY_MAIN}"), "'in'");
    test_invalid_effect(r, &format!("in half3x3 m;{EMPTY_MAIN}"), "'in'");
});

// Port of: tests/SkRuntimeEffectTest.cpp#L108-L111 (chrome/m156)
def_test!(SkRuntimeEffectInvalid_UndefinedFunction, |r| {
    test_invalid_effect(
        r,
        "half4 missing(); half4 main(float2 p) { return missing(); }",
        "function 'half4 missing()' is not defined",
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L113-L116 (chrome/m156)
def_test!(SkRuntimeEffectInvalid_UndefinedMain, |r| {
    // Shouldn't be possible to create an SkRuntimeEffect without "main"
    test_invalid_effect(r, "", "main");
});

// Port of: tests/SkRuntimeEffectTest.cpp#L118-L125 (chrome/m156)
def_test!(SkRuntimeEffectInvalid_SkCapsDisallowed, |r| {
    // sk_Caps is an internal system. It should not be visible to runtime effects
    test_invalid_effect(
        r,
        "half4 main(float2 p) { return sk_Caps.floatIs32Bits ? half4(1) : half4(0); }",
        "name 'sk_Caps' is reserved",
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L127-L148 (chrome/m156)
def_test!(SkRuntimeEffect_DeadCodeEliminationStackOverflow, |r| {
    // Verify that a deeply-nested loop does not cause stack overflow during dead-code
    // elimination.
    let result = RuntimeEffect::make_for_color_filter(
        r"
        half4 main(half4 color) {
            half value = color.r;

            for (int a=0; a<10; ++a) { // 10
            for (int b=0; b<10; ++b) { // 100
            for (int c=0; c<10; ++c) { // 1000
            for (int d=0; d<10; ++d) { // 10000
                ++value;
            }}}}

            return value.xxxx;
        }
    ",
        None,
    );
    // (`SK_BUILD_FOR_FUZZER` is not defined.)
    reporter_assert!(
        r,
        result.is_ok(),
        "{}",
        result.as_ref().err().cloned().unwrap_or_default()
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L150-L161 (chrome/m156)
def_test!(SkRuntimeEffectCanDisableES2Restrictions, |r| {
    let test_valid_es3 = |r: &mut Reporter, sksl: &str| {
        let opt = runtime_effect_priv::es3_options();
        let result = RuntimeEffect::make_for_shader(sksl, Some(&opt));
        reporter_assert!(
            r,
            result.is_ok(),
            "{}",
            result.as_ref().err().cloned().unwrap_or_default()
        );
    };

    test_invalid_effect(
        r,
        &format!("float f[2] = float[2](0, 1);{EMPTY_MAIN}"),
        "construction of array type",
    );
    test_valid_es3(r, &format!("float f[2] = float[2](0, 1);{EMPTY_MAIN}"));
});

// Port of: tests/SkRuntimeEffectTest.cpp#L163-L175 (chrome/m156)
def_test!(SkRuntimeEffectCanEnableVersion300, |r| {
    let test_valid = |r: &mut Reporter, sksl: &str| {
        let result = RuntimeEffect::make_for_shader(sksl, None);
        reporter_assert!(
            r,
            result.is_ok(),
            "{}",
            result.as_ref().err().cloned().unwrap_or_default()
        );
    };

    test_invalid_effect(
        r,
        &format!("#version 100\nfloat f[2] = float[2](0, 1);{EMPTY_MAIN}"),
        "construction of array type",
    );
    test_valid(
        r,
        &format!("#version 300\nfloat f[2] = float[2](0, 1);{EMPTY_MAIN}"),
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L177-L204 (chrome/m156)
def_test!(SkRuntimeEffectUniformFlags, |r| {
    let result = RuntimeEffect::make_for_shader(
        format!(
            r"
        uniform int simple;                      // should have no flags
        uniform float arrayOfOne[1];             // should have kArray_Flag
        uniform float arrayOfMultiple[2];        // should have kArray_Flag
        layout(color) uniform float4 color;      // should have kColor_Flag
        uniform half3 halfPrecisionFloat;        // should have kHalfPrecision_Flag
        layout(color) uniform half4 allFlags[2]; // should have Array | Color | HalfPrecision
    {EMPTY_MAIN}"
        ),
        None,
    );
    reporter_assert!(
        r,
        result.is_ok(),
        "{}",
        result.as_ref().err().cloned().unwrap_or_default()
    );
    let Ok(effect) = result else {
        return;
    };

    let uniforms = effect.uniforms();
    reporter_assert!(r, uniforms.len() == 6);

    reporter_assert!(r, uniforms[0].flags() == uniform_flags::Flags::empty());
    reporter_assert!(r, uniforms[1].flags() == uniform_flags::Flags::ARRAY);
    reporter_assert!(r, uniforms[2].flags() == uniform_flags::Flags::ARRAY);
    reporter_assert!(r, uniforms[3].flags() == uniform_flags::Flags::COLOR);
    reporter_assert!(
        r,
        uniforms[4].flags() == uniform_flags::Flags::HALF_PRECISION
    );
    reporter_assert!(
        r,
        uniforms[5].flags()
            == (uniform_flags::Flags::ARRAY
                | uniform_flags::Flags::COLOR
                | uniform_flags::Flags::HALF_PRECISION)
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L206-L218 (chrome/m156)
def_test!(SkRuntimeEffectValidation, |r| {
    let es2_effect =
        RuntimeEffect::make_for_shader(format!("#version 100\n{EMPTY_MAIN}"), None).ok();
    let es3_effect =
        RuntimeEffect::make_for_shader(format!("#version 300\n{EMPTY_MAIN}"), None).ok();
    reporter_assert!(r, es2_effect.is_some() && es3_effect.is_some());
    let (Some(es2_effect), Some(es3_effect)) = (es2_effect, es3_effect) else {
        return;
    };

    let es2_caps = Capabilities::raster_backend();
    reporter_assert!(r, es2_caps.sksl_version() == Version::K100);

    reporter_assert!(r, runtime_effect_priv::can_draw(es2_caps, &es2_effect));
    reporter_assert!(r, !runtime_effect_priv::can_draw(es2_caps, &es3_effect));
});

/// A result checker for the `test_valid`/`test_invalid` closures of the factory tests.
fn check_valid(r: &mut Reporter, result: &Result<RuntimeEffect, String>) {
    reporter_assert!(
        r,
        result.is_ok(),
        "{}",
        result.as_ref().err().cloned().unwrap_or_default()
    );
}

fn check_invalid(r: &mut Reporter, result: &Result<RuntimeEffect, String>, expected: &str) {
    reporter_assert!(r, result.is_err());
    let error_text = result.as_ref().err().cloned().unwrap_or_default();
    reporter_assert!(
        r,
        error_text.contains(expected),
        "Expected error message to contain \"{}\". Actual message: \"{}\"",
        expected,
        error_text
    );
}

// Port of: tests/SkRuntimeEffectTest.cpp#L220-L271 (chrome/m156)
def_test!(SkRuntimeEffectForColorFilter, |r| {
    // Tests that the color filter factory rejects or accepts certain SkSL constructs
    let test_valid = |r: &mut Reporter, sksl: &str| {
        check_valid(r, &RuntimeEffect::make_for_color_filter(sksl, None));
    };

    let test_invalid = |r: &mut Reporter, sksl: &str, expected: &str| {
        check_invalid(
            r,
            &RuntimeEffect::make_for_color_filter(sksl, None),
            expected,
        );
    };

    // Color filters must use the 'half4 main(half4)' signature. Either color can be float4/vec4
    test_valid(r, "half4  main(half4  c) { return c; }");
    test_valid(r, "float4 main(half4  c) { return c; }");
    test_valid(r, "half4  main(float4 c) { return c; }");
    test_valid(r, "float4 main(float4 c) { return c; }");
    test_valid(r, "vec4   main(half4  c) { return c; }");
    test_valid(r, "half4  main(vec4   c) { return c; }");
    test_valid(r, "vec4   main(vec4   c) { return c; }");

    // Invalid return types
    test_invalid(r, "void  main(half4 c) {}", "'main' must return");
    test_invalid(
        r,
        "half3 main(half4 c) { return c.rgb; }",
        "'main' must return",
    );

    // Invalid argument types (some are valid as shaders, but not color filters)
    test_invalid(r, "half4 main() { return half4(1); }", "'main' parameter");
    test_invalid(
        r,
        "half4 main(float2 p) { return half4(1); }",
        "'main' parameter",
    );
    test_invalid(
        r,
        "half4 main(float2 p, half4 c) { return c; }",
        "'main' parameter",
    );

    // sk_FragCoord should not be available
    test_invalid(
        r,
        "half4 main(half4 c) { return sk_FragCoord.xy01; }",
        "unknown identifier",
    );

    // Sampling a child shader requires that we pass explicit coords
    test_valid(
        r,
        "uniform shader child;half4 main(half4 c) { return child.eval(c.rg); }",
    );

    // Sampling a colorFilter requires a color
    test_valid(
        r,
        "uniform colorFilter child;half4 main(half4 c) { return child.eval(c); }",
    );

    // Sampling a blender requires two colors
    test_valid(
        r,
        "uniform blender child;half4 main(half4 c) { return child.eval(c, c); }",
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L273-L332 (chrome/m156)
def_test!(SkRuntimeEffectForBlender, |r| {
    // Tests that the blender factory rejects or accepts certain SkSL constructs
    let test_valid = |r: &mut Reporter, sksl: &str| {
        check_valid(r, &RuntimeEffect::make_for_blender(sksl, None));
    };

    let test_invalid = |r: &mut Reporter, sksl: &str, expected: &str| {
        check_invalid(r, &RuntimeEffect::make_for_blender(sksl, None), expected);
    };

    // Blenders must use the 'half4 main(half4, half4)' signature. Any mixture of
    // float4/vec4/half4 is allowed.
    test_valid(r, "half4  main(half4  s, half4  d) { return s; }");
    test_valid(r, "float4 main(float4 s, float4 d) { return d; }");
    test_valid(r, "float4 main(half4  s, float4 d) { return s; }");
    test_valid(r, "half4  main(float4 s, half4  d) { return d; }");
    test_valid(r, "vec4   main(half4  s, half4  d) { return s; }");
    test_valid(r, "half4  main(vec4   s, vec4   d) { return d; }");
    test_valid(r, "vec4   main(vec4   s, vec4   d) { return s; }");

    // Invalid return types
    test_invalid(r, "void  main(half4 s, half4 d) {}", "'main' must return");
    test_invalid(
        r,
        "half3 main(half4 s, half4 d) { return s.rgb; }",
        "'main' must return",
    );

    // Invalid argument types (some are valid as shaders/color filters)
    test_invalid(r, "half4 main() { return half4(1); }", "'main' parameter");
    test_invalid(r, "half4 main(half4 c) { return c; }", "'main' parameter");
    test_invalid(
        r,
        "half4 main(float2 p) { return half4(1); }",
        "'main' parameter",
    );
    test_invalid(
        r,
        "half4 main(float2 p, half4 c) { return c; }",
        "'main' parameter",
    );
    test_invalid(
        r,
        "half4 main(float2 p, half4 a, half4 b) { return a; }",
        "'main' parameter",
    );
    test_invalid(
        r,
        "half4 main(half4 a, half4 b, half4 c) { return a; }",
        "'main' parameter",
    );

    // sk_FragCoord should not be available
    test_invalid(
        r,
        "half4 main(half4 s, half4 d) { return sk_FragCoord.xy01; }",
        "unknown identifier",
    );

    // Sampling a child shader requires that we pass explicit coords
    test_valid(
        r,
        "uniform shader child;half4 main(half4 s, half4 d) { return child.eval(s.rg); }",
    );

    // Sampling a colorFilter requires a color
    test_valid(
        r,
        "uniform colorFilter child;half4 main(half4 s, half4 d) { return child.eval(d); }",
    );

    // Sampling a blender requires two colors
    test_valid(
        r,
        "uniform blender child;half4 main(half4 s, half4 d) { return child.eval(s, d); }",
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L334-L407 (chrome/m156)
def_test!(SkRuntimeEffectForShader, |r| {
    // Tests that the shader factory rejects or accepts certain SkSL constructs
    let test_valid = |r: &mut Reporter, sksl: &str, options: Options<'_>| {
        check_valid(r, &RuntimeEffect::make_for_shader(sksl, Some(&options)));
    };

    // (The C++ lambda takes `options` but compiles with the default ones, as written.)
    let test_invalid = |r: &mut Reporter, sksl: &str, expected: &str, _options: Options<'_>| {
        check_invalid(r, &RuntimeEffect::make_for_shader(sksl, None), expected);
    };

    // Shaders must use the 'half4 main(float2)' signature
    // Either color can be half4/float4/vec4, but the coords must be float2/vec2
    test_valid(
        r,
        "half4  main(float2 p) { return p.xyxy; }",
        Options::default(),
    );
    test_valid(
        r,
        "float4 main(float2 p) { return p.xyxy; }",
        Options::default(),
    );
    test_valid(
        r,
        "vec4   main(float2 p) { return p.xyxy; }",
        Options::default(),
    );
    test_valid(
        r,
        "half4  main(vec2   p) { return p.xyxy; }",
        Options::default(),
    );
    test_valid(
        r,
        "vec4   main(vec2   p) { return p.xyxy; }",
        Options::default(),
    );

    // The 'half4 main(float2, half4|float4)' signature is disallowed on both public and private
    // runtime effects.
    let mut options = Options::default();
    runtime_effect_priv::allow_private_access(&mut options);
    for sksl in [
        "half4  main(float2 p, half4  c) { return c; }",
        "half4  main(float2 p, float4 c) { return c; }",
        "half4  main(float2 p, vec4   c) { return c; }",
        "float4 main(float2 p, half4  c) { return c; }",
        "vec4   main(float2 p, half4  c) { return c; }",
        "vec4   main(vec2   p, vec4   c) { return c; }",
    ] {
        test_invalid(r, sksl, "'main' parameter", Options::default());
        test_invalid(r, sksl, "'main' parameter", options);
    }

    // Invalid return types
    test_invalid(
        r,
        "void  main(float2 p) {}",
        "'main' must return",
        Options::default(),
    );
    test_invalid(
        r,
        "half3 main(float2 p) { return p.xy1; }",
        "'main' must return",
        Options::default(),
    );

    // Invalid argument types (some are valid as color filters, but not shaders)
    test_invalid(
        r,
        "half4 main() { return half4(1); }",
        "'main' parameter",
        Options::default(),
    );
    test_invalid(
        r,
        "half4 main(half4 c) { return c; }",
        "'main' parameter",
        Options::default(),
    );

    // sk_FragCoord should be available, but only if we've enabled it via Options
    test_invalid(
        r,
        "half4 main(float2 p) { return sk_FragCoord.xy01; }",
        "unknown identifier 'sk_FragCoord'",
        Options::default(),
    );

    test_valid(
        r,
        "half4 main(float2 p) { return sk_FragCoord.xy01; }",
        options,
    );

    // Sampling a child shader requires that we pass explicit coords
    test_valid(
        r,
        "uniform shader child;half4 main(float2 p) { return child.eval(p); }",
        Options::default(),
    );

    // Sampling a colorFilter requires a color
    test_valid(
        r,
        "uniform colorFilter child;half4 main(float2 p) { return child.eval(half4(1)); }",
        Options::default(),
    );

    // Sampling a blender requires two colors
    test_valid(
        r,
        "uniform blender child;half4 main(float2 p) { return child.eval(half4(0.5), half4(0.6)); }",
        Options::default(),
    );
});

/// `PreTestFn`.
type PreTestFn<'a> = &'a dyn Fn(&Canvas, &mut Paint);

// Port of: tests/SkRuntimeEffectTest.cpp#L409-L418 (chrome/m156)
fn paint_canvas(canvas: &Canvas, paint: &mut Paint, pre_test_callback: Option<PreTestFn<'_>>) {
    canvas.save();
    if let Some(pre_test_callback) = pre_test_callback {
        pre_test_callback(canvas, paint);
    }
    canvas.draw_paint(paint);
    canvas.restore();
}

// Port of: tests/SkRuntimeEffectTest.cpp#L420-L426 (chrome/m156)
fn read_pixels(surface: &mut dyn TestSurface, pixels: &mut [u32; 4]) -> bool {
    let info = surface.image_info();
    // `SkPixmap dest{info, pixels, info.minRowBytes()}`: the pixels are read as native `uint32_t`s.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&info, None);
    let ok = surface.read_pixels(&mut bitmap);
    let Some(pixmap) = bitmap.peek_pixels() else {
        return false;
    };
    let Some(bytes) = pixmap.addr() else {
        return false;
    };
    for (pixel, chunk) in pixels.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *pixel = u32::from_ne_bytes(*chunk);
    }
    ok
}

// Port of: tests/SkRuntimeEffectTest.cpp#L428-L449 (chrome/m156)
fn verify_2x2_surface_results(
    r: &mut Reporter,
    effect: &RuntimeEffect,
    surface: &mut dyn TestSurface,
    expected: [u32; 4],
) {
    let mut actual = [0_u32; 4];
    if !read_pixels(surface, &mut actual) {
        errorf!(r, "readPixels: readPixels failed");
        return;
    }

    if actual != expected {
        errorf!(
            r,
            "Runtime effect didn't match expectations\n\
             Expected: [ {:08x} {:08x} {:08x} {:08x} ]\n\
             Got     : [ {:08x} {:08x} {:08x} {:08x} ]\n\
             SkSL:\n{}\n",
            expected[0],
            expected[1],
            expected[2],
            expected[3],
            actual[0],
            actual[1],
            actual[2],
            actual[3],
            effect.source()
        );
    }
}

// Port of: tests/SkRuntimeEffectTest.cpp#L451-L468 (chrome/m156)
fn make_surface(size: (i32, i32)) -> Surface<'static> {
    let info = ImageInfo::new(size, ColorType::RGBA8888, AlphaType::Premul, None);
    surfaces::raster(&info, None, None).expect("a raster surface")
}

/// `TestEffect`.
// Port of: tests/SkRuntimeEffectTest.cpp#L470-L558 (chrome/m156)
struct TestEffect<'a> {
    surface: Box<dyn TestSurface + 'a>,
    builder: Option<RuntimeShaderBuilder>,
}

impl TestEffect<'static> {
    fn new() -> Self {
        Self::with_size((2, 2))
    }

    /// `TestEffect(r, grContext, graphite, size)`: a raster surface of `size`.
    fn with_size(size: (i32, i32)) -> Self {
        Self::with_surface(Box::new(make_surface(size)))
    }
}

impl<'a> TestEffect<'a> {
    /// `TestEffect(r, grContext, graphite, size)` for a surface the caller made (a raster one, or
    /// the Graphite one of `SkRuntimeEffectSimple_Graphite`).
    fn with_surface(surface: Box<dyn TestSurface + 'a>) -> Self {
        TestEffect {
            surface,
            builder: None,
        }
    }

    /// `trace`: draws the traced copy of the built effect's shader on the surface, and returns the
    /// dump of its debug trace.
    fn trace(&mut self, r: &mut Reporter, trace_coord: IPoint) -> String {
        let Some(shader) = self.builder().make_shader(None) else {
            errorf!(r, "Effect didn't produce a shader");
            return String::new();
        };

        let Some(traced) = RuntimeEffect::make_traced(&shader, trace_coord) else {
            errorf!(r, "Effect didn't produce a traced shader");
            return String::new();
        };

        let canvas = self.surface.canvas();
        let mut paint = Paint::default();
        paint.set_shader(traced.shader);
        paint.set_blend_mode(BlendMode::Src);

        paint_canvas(canvas, &mut paint, None);

        traced.debug_trace.dump()
    }

    fn build(&mut self, r: &mut Reporter, src: &str) {
        let mut options = Options::default();
        runtime_effect_priv::allow_private_access(&mut options);
        match RuntimeEffect::make_for_shader(src, Some(&options)) {
            Ok(effect) => self.builder = Some(RuntimeShaderBuilder::new(effect)),
            Err(error_text) => {
                errorf!(r, "Effect didn't compile: {}", error_text);
            }
        }
    }

    fn builder(&mut self) -> &mut RuntimeShaderBuilder {
        self.builder.as_mut().expect("a built effect")
    }

    fn uniform_f32(&mut self, name: &str, val: &[f32]) {
        self.builder().uniform(name).set_f32(val);
    }

    fn uniform_i32(&mut self, name: &str, val: &[i32]) {
        self.builder().uniform(name).set_i32(val);
    }

    fn child_null(&mut self, name: &str) {
        self.builder().child(name).assign_null();
    }

    fn child(&mut self, name: &str, shader: Shader) {
        self.builder().child(name).assign(shader);
    }

    fn test(
        &mut self,
        r: &mut Reporter,
        expected: [u32; 4],
        pre_test_callback: Option<PreTestFn<'_>>,
    ) {
        let Some(shader) = self.builder().make_shader(None) else {
            errorf!(r, "Effect didn't produce a shader");
            return;
        };

        // We shouldn't need to clear the canvas, because we are about to paint over the whole
        // thing with a `source` blend mode. However, there are a few devices where the
        // background can leak through when we paint with MSAA on. (This seems to be a
        // driver/hardware bug.) Graphite, at present, uses MSAA to do `drawPaint`. To avoid
        // flakiness in this test on those devices, we explicitly clear the canvas here.
        // (skbug.com/40044848)
        let canvas = self.surface.canvas();
        canvas.clear(Color::BLACK);

        let mut paint = Paint::default();
        paint.set_shader(shader);
        paint.set_blend_mode(BlendMode::Src);

        paint_canvas(canvas, &mut paint, pre_test_callback);

        let effect = self.builder().effect().clone();
        verify_2x2_surface_results(r, &effect, &mut *self.surface, expected);
    }

    fn test_uniform(
        &mut self,
        r: &mut Reporter,
        expected: u32,
        pre_test_callback: Option<PreTestFn<'_>>,
    ) {
        self.test(r, [expected; 4], pre_test_callback);
    }
}

// Produces a shader which will paint these opaque colors in a 2x2 rectangle:
// [  Red, Green ]
// [ Blue, White ]
// Port of: tests/SkRuntimeEffectTest.cpp#L690-L700 (chrome/m156)
fn make_rgbw_shader() -> Shader {
    let colors = [
        colors::WHITE,
        colors::WHITE,
        colors::BLUE,
        colors::BLUE,
        colors::RED,
        colors::RED,
        colors::GREEN,
        colors::GREEN,
    ];
    let pos = [0.0, 0.25, 0.25, 0.50, 0.50, 0.75, 0.75, 1.0];
    assert_eq!(colors.len(), pos.len(), "size mismatch");
    gradient_shaders::sweep_gradient(
        (1.0, 1.0),
        (0.0, 360.0),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
    .expect("a sweep gradient")
}

// Port of: tests/SkRuntimeEffectTest.cpp#L702-L808 (chrome/m156)
// The C++ builds its `TestEffect` from `grContext` and `graphite`; here the caller passes the
// surface it made (raster for `SkRuntimeEffectSimple`, Graphite for the Graphite variant).
fn test_runtime_effect_shaders(r: &mut Reporter, surface: Box<dyn TestSurface + '_>) {
    let mut effect = TestEffect::with_surface(surface);

    // Local coords
    effect.build(
        r,
        "half4 main(float2 p) { return half4(half2(p - 0.5), 0, 1); }",
    );
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_00FF, 0xFF00_FF00, 0xFF00_FFFF],
        None,
    );

    // Use of a simple uniform. (Draw twice with two values to ensure it's updated).
    effect.build(
        r,
        "uniform float4 gColor; half4 main(float2 p) { return half4(gColor); }",
    );
    effect.uniform_f32("gColor", &[0.0, 0.25, 0.75, 1.0]);
    effect.test_uniform(r, 0xFFBF_4000, None);
    effect.uniform_f32("gColor", &[1.0, 0.0, 0.0, 0.498]);
    effect.test_uniform(r, 0x7F00_00FF, None); // Tests that we don't clamp to valid premul

    // Same, with integer uniforms
    effect.build(
        r,
        "uniform int4 gColor; half4 main(float2 p) { return half4(gColor) / 255.0; }",
    );
    effect.uniform_i32("gColor", &[0x00, 0x40, 0xBF, 0xFF]);
    effect.test_uniform(r, 0xFFBF_4000, None);
    effect.uniform_i32("gColor", &[0xFF, 0x00, 0x00, 0x7F]);
    effect.test_uniform(r, 0x7F00_00FF, None); // Tests that we don't clamp to valid premul

    // Test sk_FragCoord (device coords). Rotate the canvas to be sure we're seeing device
    // coords. Since the surface is 2x2, we should see (0,0), (1,0), (0,1), (1,1). Multiply by
    // 0.498 to make sure we're not saturating unexpectedly.
    effect.build(
        r,
        "half4 main(float2 p) { return half4(0.498 * (half2(sk_FragCoord.xy) - 0.5), 0, 1); }",
    );
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_007F, 0xFF00_7F00, 0xFF00_7F7F],
        Some(&|canvas, _| {
            canvas.rotate(45.0, None);
        }),
    );

    // Runtime effects should use relaxed precision rules by default
    effect.build(r, "half4 main(float2 p) { return float4(p - 0.5, 0, 1); }");
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_00FF, 0xFF00_FF00, 0xFF00_FFFF],
        None,
    );

    // ... and support *returning* float4 (aka vec4), not just half4
    effect.build(r, "float4 main(float2 p) { return float4(p - 0.5, 0, 1); }");
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_00FF, 0xFF00_FF00, 0xFF00_FFFF],
        None,
    );
    effect.build(r, "vec4 main(float2 p) { return float4(p - 0.5, 0, 1); }");
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_00FF, 0xFF00_FF00, 0xFF00_FFFF],
        None,
    );

    // Mutating coords should work. (skbug.com/40042292)
    effect.build(r, "vec4 main(vec2 p) { p -= 0.5; return vec4(p, 0, 1); }");
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_00FF, 0xFF00_FF00, 0xFF00_FFFF],
        None,
    );
    effect.build(
        r,
        "void moveCoords(inout vec2 p) { p -= 0.5; }\
         vec4 main(vec2 p) { moveCoords(p); return vec4(p, 0, 1); }",
    );
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_00FF, 0xFF00_FF00, 0xFF00_FFFF],
        None,
    );

    //
    // Sampling children
    //

    // Sampling a null shader should return transparent black
    // (The `!graphite` guard is true here.)
    effect.build(
        r,
        "uniform shader child;half4 main(float2 p) { return child.eval(p); }",
    );
    effect.child_null("child");
    effect.test_uniform(
        r,
        0x0000_0000,
        Some(&|_, paint| {
            paint.set_color4f(Color4f::new(1.0, 1.0, 0.0, 1.0), None);
        }),
    );

    // Sampling a null color-filter should return the passed-in color
    effect.build(
        r,
        "uniform colorFilter child;\
         half4 main(float2 p) { return child.eval(half4(1, 1, 0, 1)); }",
    );
    effect.child_null("child");
    effect.test_uniform(r, 0xFF00_FFFF, None);

    // Sampling a null blender should return blend_src_over(src, dest).
    effect.build(
        r,
        "uniform blender child;\
         half4 main(float2 p) {\
             float4 src = float4(p - 0.5, 0, 1) * 0.498;\
             return child.eval(src, half4(0, 0, 0, 1));\
         }",
    );
    effect.child_null("child");
    effect.test(
        r,
        [0xFF00_0000, 0xFF00_007F, 0xFF00_7F00, 0xFF00_7F7F],
        None,
    );

    // Sampling a simple child at our coordinates
    let rgbw_shader = make_rgbw_shader();

    effect.build(
        r,
        "uniform shader child;half4 main(float2 p) { return child.eval(p); }",
    );
    effect.child("child", rgbw_shader.clone());
    effect.test(
        r,
        [0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000, 0xFFFF_FFFF],
        None,
    );

    // Sampling with explicit coordinates (reflecting about the diagonal)
    effect.build(
        r,
        "uniform shader child;half4 main(float2 p) { return child.eval(p.yx); }",
    );
    effect.child("child", rgbw_shader.clone());
    effect.test(
        r,
        [0xFF00_00FF, 0xFFFF_0000, 0xFF00_FF00, 0xFFFF_FFFF],
        None,
    );

    // Bind an image shader, but don't use it - ensure that we don't assert or generate bad
    // shaders. (skbug.com/40043510)
    effect.build(
        r,
        "uniform shader child;half4 main(float2 p) { return half4(0, 1, 0, 1); }",
    );
    effect.child("child", rgbw_shader.clone());
    effect.test_uniform(r, 0xFF00_FF00, None);

    //
    // Helper functions
    //

    // Test case for inlining in the pipeline-stage and fragment-shader passes
    // (skbug.com/40041860):
    effect.build(
        r,
        "float2 helper(float2 x) { return x + 1; }\
         half4 main(float2 p) { float2 v = helper(p); return half4(half2(v), 0, 1); }",
    );
    effect.test_uniform(r, 0xFF00_FFFF, None);

    // Passing a shader to a helper function
    effect.build(
        r,
        "uniform shader child; float2 position;\
         noinline half4 my_eval(shader s) { return s.eval(position); }\
         half4 main(float2 p) { position = p; return my_eval(child); }",
    );
    effect.child("child", rgbw_shader);
    effect.test(
        r,
        [0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000, 0xFFFF_FFFF],
        None,
    );
}

// Port of: tests/SkRuntimeEffectTest.cpp#L810-L812 (chrome/m156)
def_test!(SkRuntimeEffectSimple, |r| {
    test_runtime_effect_shaders(r, Box::new(make_surface((2, 2))));
});

// Port of: tests/SkRuntimeEffectTest.cpp#L742-L748 (chrome/m156)
def_graphite_adapter_test!(SkRuntimeEffectSimple_Graphite, |reporter, context| {
    let recorder = context.make_recorder(None);
    let info = ImageInfo::new((2, 2), ColorType::RGBA8888, AlphaType::Premul, None);
    // `make_surface` with a Graphite `GraphiteInfo`: `SkSurfaces::RenderTarget(recorder, info)`.
    let surface = GraphiteSurface::render_target(&recorder, &info, Mipmapped::No, None, "");
    reporter_assert!(reporter, surface.is_some());
    let Some(surface) = surface else {
        return;
    };
    test_runtime_effect_shaders(
        reporter,
        Box::new(GraphiteTestSurface {
            context,
            surface: &surface,
        }),
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L956-L972 (chrome/m156)
def_test!(SkRuntimeColorFilterLimitedToES2, |r| {
    // Verify that SkSL requesting #version 300 can't be used to create a color-filter effect.
    // This restriction could be removed if we can find a way to implement filterColor4f for
    // these color filters.
    {
        let effect = RuntimeEffect::make_for_color_filter(
            r"
            #version 300
            half4 main(half4 inColor) { return half4(1, 0, 0, 1); }
        ",
            None,
        );
        reporter_assert!(r, effect.is_err());
    }

    {
        let effect = RuntimeEffect::make_for_color_filter(
            r"
            #version 300
            uniform int loops;
            half4 main(half4 inColor) {
                half4 result = half4(1, 0, 0, 1);
                for (int i = 0; i < loops; i++) {
                    result = result.argb;
                }
                return result;
            }
        ",
            None,
        );
        reporter_assert!(r, effect.is_err());
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1176-L1193 (chrome/m156)
def_test!(SkRuntimeShaderBuilderReuse, |r| {
    let source = r"
        uniform half x;
        half4 main(float2 p) { return half4(x); }
    ";

    let effect = RuntimeEffect::make_for_shader(source, None).ok();
    reporter_assert!(r, effect.is_some());
    let Some(effect) = effect else {
        return;
    };

    // Test passes if this sequence doesn't assert.  skbug.com/40042013
    let mut b = RuntimeShaderBuilder::new(effect);
    b.uniform("x").set_f32(&[0.0]);
    let _shader_0 = b.make_shader(None);

    b.uniform("x").set_f32(&[1.0]);
    let _shader_1 = b.make_shader(None);
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1213-L1241 (chrome/m156)
def_test!(SkRuntimeShaderBuilderSetUniforms, |r| {
    let source = r"
        uniform half x;
        uniform vec2 offset;
        half4 main(float2 p) { return half4(x); }
    ";

    let effect = RuntimeEffect::make_for_shader(source, None).ok();
    reporter_assert!(r, effect.is_some());
    let Some(effect) = effect else {
        return;
    };

    let mut b = RuntimeShaderBuilder::new(effect);

    // Test passes if this sequence doesn't assert.
    let x = 1.0_f32;
    reporter_assert!(r, b.uniform("x").set_f32(&[x]));

    // add extra value to ensure that set doesn't try to use sizeof(array)
    let origin = [2.0_f32, 3.0, 4.0];
    reporter_assert!(r, b.uniform("offset").set_f32(&origin[..2]));

    // (`SK_DEBUG` is not defined: the C++ asserts in debug builds, so the failing sets are
    // only checked there when it is not.)
    reporter_assert!(r, !b.uniform("offset").set_f32(&origin[..1]));
    reporter_assert!(r, !b.uniform("offset").set_f32(&origin[..3]));

    let _shader = b.make_shader(None);
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1243-L1262 (chrome/m156)
def_test!(SkRuntimeEffectThreaded, |r| {
    // This tests that we can safely use SkRuntimeEffect::MakeForShader from more than one
    // thread, and also that programs don't refer to shared structures owned by the compiler.
    // skbug.com/40041933
    const K_SOURCE: &str = "half4 main(float2 p) { return sk_FragCoord.xyxy; }";

    let results: Vec<bool> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    let mut options = Options::default();
                    runtime_effect_priv::allow_private_access(&mut options);
                    RuntimeEffect::make_for_shader(K_SOURCE, Some(&options)).is_ok()
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().expect("a thread"))
            .collect()
    });
    for effect_made in results {
        reporter_assert!(r, effect_made);
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1264-L1306 (chrome/m156)
def_test!(SkRuntimeEffectAllowsPrivateAccess, |r| {
    let default_options = Options::default();
    let mut options_with_access = Options::default();
    runtime_effect_priv::allow_private_access(&mut options_with_access);

    // Confirm that shaders can only access $private_functions when private access is allowed.
    {
        const K_SHADER: &str = "half4 main(float2 p) { return $hsl_to_rgb(p.xxx, p.y); }";
        let normal = RuntimeEffect::make_for_shader(K_SHADER, Some(&default_options));
        reporter_assert!(r, normal.is_err());
        let privileged = RuntimeEffect::make_for_shader(K_SHADER, Some(&options_with_access));
        reporter_assert!(
            r,
            privileged.is_ok(),
            "{}",
            privileged.as_ref().err().cloned().unwrap_or_default()
        );
    }

    // Confirm that color filters can only access $private_functions when private access is
    // allowed.
    {
        const K_COLOR_FILTER: &str = "half4 main(half4 c)  { return $hsl_to_rgb(c.rgb, c.a); }";
        let normal = RuntimeEffect::make_for_color_filter(K_COLOR_FILTER, Some(&default_options));
        reporter_assert!(r, normal.is_err());
        let privileged =
            RuntimeEffect::make_for_color_filter(K_COLOR_FILTER, Some(&options_with_access));
        reporter_assert!(
            r,
            privileged.is_ok(),
            "{}",
            privileged.as_ref().err().cloned().unwrap_or_default()
        );
    }

    // Confirm that blenders can only access $private_functions when private access is allowed.
    {
        const K_BLENDER: &str = "half4 main(half4 s, half4 d) { return $hsl_to_rgb(s.rgb, d.a); }";
        let normal = RuntimeEffect::make_for_blender(K_BLENDER, Some(&default_options));
        reporter_assert!(r, normal.is_err());
        let privileged = RuntimeEffect::make_for_blender(K_BLENDER, Some(&options_with_access));
        reporter_assert!(
            r,
            privileged.is_ok(),
            "{}",
            privileged.as_ref().err().cloned().unwrap_or_default()
        );
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1329-L1359 (chrome/m156)
def_test!(SkRuntimeStructNameReuse, |r| {
    // Test that two different runtime effects can reuse struct names in a single paint
    // operation
    let child_effect = RuntimeEffect::make_for_shader(
        "uniform shader paint;\
         struct S { half4 rgba; };\
         void process(inout S s) { s.rgba.rgb *= 0.5; }\
         half4 main(float2 p) { S s; s.rgba = paint.eval(p); process(s); return s.rgba; }",
        None,
    );
    reporter_assert!(
        r,
        child_effect.is_ok(),
        "{}\n",
        child_effect.as_ref().err().cloned().unwrap_or_default()
    );
    let Ok(child_effect) = child_effect else {
        return;
    };
    let source_color = shaders::color_in_space(Color4f::new(0.99608, 0.50196, 0.0, 1.0), None)
        .expect("a finite color");
    const K_EXPECTED: u32 = 0xFF00_407F;
    let child = child_effect.make_shader(
        skia_rust_core::data::Data::new_empty(),
        &[source_color.into()],
        None,
    );
    reporter_assert!(r, child.is_some());
    let Some(child) = child else {
        return;
    };

    let mut effect = TestEffect::new();
    effect.build(
        r,
        "uniform shader child;\
         struct S { float2 coord; };\
         void process(inout S s) { s.coord = s.coord.yx; }\
         half4 main(float2 p) { S s; s.coord = p; process(s); return child.eval(s.coord); }",
    );
    effect.child("child", child);
    effect.test_uniform(r, K_EXPECTED, Some(&|_, _| {}));
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1516-L1594 (chrome/m156)
def_test!(SkRuntimeShaderIsOpaque, |r| {
    // This test verifies that we detect certain simple patterns in runtime shaders, and can
    // deduce (via code in SkSL::Analysis::ReturnsOpaqueColor) that the resulting shader is
    // always opaque. That logic is conservative, and the tests below reflect this.

    let test = |r: &mut Reporter, body: &str, expect_opaque: bool| {
        let result = RuntimeEffect::make_for_shader(
            format!(
                r"
            uniform shader cOnes;
            uniform shader cZeros;
            uniform float4 uOnes;
            uniform float4 uZeros;
            half4 main(float2 xy) {{
                {body}
            }}"
            ),
            None,
        );
        reporter_assert!(r, result.is_ok());
        let Ok(effect) = result else {
            return;
        };

        let c_ones = shaders::color(Color::WHITE);
        let c_zeros = shaders::color(Color::TRANSPARENT);
        assert!(c_ones.is_opaque());
        assert!(!c_zeros.is_opaque());

        let mut builder = RuntimeShaderBuilder::new(effect);
        builder.child("cOnes").assign(c_ones);
        builder.child("cZeros").assign(c_zeros);
        builder.uniform("uOnes").set_f32(&colors::WHITE.as_array());
        builder
            .uniform("uZeros")
            .set_f32(&colors::TRANSPARENT.as_array());

        let shader = builder.make_shader(None);
        reporter_assert!(r, shader.is_some());
        if let Some(shader) = shader {
            reporter_assert!(r, shader.is_opaque() == expect_opaque);
        }
    };

    // Cases where our optimization is valid, and works:

    // Returning opaque literals
    test(r, "return half4(1);", true);
    test(r, "return half4(0, 1, 0, 1);", true);
    test(r, "return half4(0, 0, 0, 1);", true);

    // Simple expressions involving uniforms
    test(r, "return uZeros.rgb1;", true);
    test(r, "return uZeros.bgra.rgb1;", true);
    test(r, "return half4(uZeros.rgb, 1);", true);

    // Simple expressions involving child.eval
    test(r, "return cZeros.eval(xy).rgb1;", true);
    test(r, "return cZeros.eval(xy).bgra.rgb1;", true);
    test(r, "return half4(cZeros.eval(xy).rgb, 1);", true);

    // Multiple returns
    test(
        r,
        "if (xy.x < 100) { return uZeros.rgb1; } else { return cZeros.eval(xy).rgb1; }",
        true,
    );

    // More expression cases:
    test(r, "return (cZeros.eval(xy) * uZeros).rgb1;", true);
    test(r, "return half4(1, 1, 1, 0.5 + 0.5);", true);

    // Constant variable propagation
    test(r, "const half4 kWhite = half4(1); return kWhite;", true);

    // Cases where our optimization is not valid, and does not happen:

    // Returning non-opaque literals
    test(r, "return half4(0);", false);
    test(r, "return half4(1, 1, 1, 0);", false);

    // Returning non-opaque uniforms or children
    test(r, "return uZeros;", false);
    test(r, "return cZeros.eval(xy);", false);

    // Multiple returns
    test(
        r,
        "if (xy.x < 100) { return uZeros; } else { return cZeros.eval(xy).rgb1; }",
        false,
    );
    test(
        r,
        "if (xy.x < 100) { return uZeros.rgb1; } else { return cZeros.eval(xy); }",
        false,
    );

    // There should (must) not be any false-positive cases. There are false-negatives.
    // In these cases, our optimization would be valid, but does not happen:

    // More complex expressions that can't be simplified
    test(
        r,
        "return xy.x < 100 ? uZeros.rgb1 : cZeros.eval(xy).rgb1;",
        false,
    );

    // Finally, there are cases that are conditional on the uniforms and children. These *could*
    // determine dynamically if the uniform and/or child being referenced is opaque, and use
    // that information. Today, we don't do this, so we pessimistically assume they're
    // transparent:
    test(r, "return uOnes;", false);
    test(r, "return cOnes.eval(xy);", false);
});

// This test verifies that when a runtime shader's input coordinates are previously transformed
// by a local matrix (which may be lifted to the vertex shader on GPU backends), the coordinates
// resolve correctly for the runtime shader and any child shaders.
// Port of: tests/SkRuntimeEffectTest.cpp#L1621-L1699 (chrome/m156)
//
// The C++ makes `surface` from `graphiteInfo` (its recorder) and makes the texture image of the
// bitmap with `ToolUtils::MakeTextureImage(canvas, ...)`, whose Graphite branch is
// `SkImages::TextureFromImage(canvas->recorder(), ...)`. The caller passes the recorder and the
// surface it made.
fn test_using_transformed_coords(
    reporter: &mut Reporter,
    recorder: &Recorder,
    surface: &mut dyn TestSurface,
) {
    // Make a 1x12 pixel image with left 1/4 red and right 3/4 green.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((12, 1), true);
    {
        let Some(bitmap_canvas) = Canvas::from_bitmap(&mut bitmap, None) else {
            return;
        };
        let mut red = Paint::default();
        red.set_color4f(colors::RED, None::<&ColorSpace>);
        bitmap_canvas.draw_irect(IRect::from_xywh(0, 0, 3, 1), &red);
        let mut green = Paint::default();
        green.set_color4f(colors::GREEN, None::<&ColorSpace>);
        bitmap_canvas.draw_irect(IRect::from_xywh(3, 0, 9, 1), &green);
    }

    // `ToolUtils::MakeTextureImage(canvas, bitmap.asImage())->makeShader(SkFilterMode::kNearest)`.
    let Some(image) = bitmap.as_image() else {
        return;
    };
    let texture = texture_from_image(recorder, &image, RequiredProperties { mipmapped: false });
    let Some(image_shader) = ImageShader::make(
        texture,
        TileMode::Clamp,
        TileMode::Clamp,
        &SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
        None,
        false,
    ) else {
        return;
    };

    // Runtime effect that sets the blue channel to 1 in the right half of its child. (The C++
    // comment "round() doesn't seem to be legal in runtime shaders" is not part of the source.)
    let src = "uniform shader s;\
        half4 main(float2 p) {\
            return half4(s.eval(p).rg, max(0.0, sign(p.x / 12.0 - 0.5)), 1.0);\
        }";
    // The C++ makes the effect twice (`runtimeEffectResult` and `effect`); both are the same.
    let effect = RuntimeEffect::make_for_shader(src, None);
    reporter_assert!(reporter, effect.is_ok());
    let Ok(effect) = effect else {
        return;
    };

    // Nest the image shader under the runtime shader, all under a local matrix transformation that
    // translates the draw right 1/4 of the way.
    let Some(nested) =
        effect.make_shader(Data::new_empty(), &[ChildPtr::Shader(image_shader)], None)
    else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_shader(nested.with_local_matrix(&Matrix::translate((3.0, 0.0))));

    surface.canvas().draw_paint(&paint);

    // Read pixels.
    let mut read_bitmap = Bitmap::new();
    read_bitmap.alloc_pixels_info(&surface.image_info(), None);
    if !surface.read_pixels(&mut read_bitmap) {
        errorf!(reporter, "readPixels failed");
        return;
    }
    let Some(pixmap) = read_bitmap.peek_pixels() else {
        return;
    };

    // The first half of the canvas should be red, since the image was drawn shifted to the right
    // with clamp tiling.
    reporter_assert!(reporter, pixmap.get_color_4f((1, 0)) == colors::RED);
    reporter_assert!(reporter, pixmap.get_color_4f((4, 0)) == colors::RED);

    // The third quarter of the canvas should be green. This is the second quarter of the image,
    // translated right, and not affected by the runtime shader which should only touch the right
    // half of the image.
    reporter_assert!(reporter, pixmap.get_color_4f((7, 0)) == colors::GREEN);

    // The last quarter of the canvas should be cyan, since the green in the image has its blue
    // channel set to 1 by the runtime shader.
    reporter_assert!(reporter, pixmap.get_color_4f((10, 0)) == colors::CYAN);
}

// Port of: tests/SkRuntimeEffectTest.cpp#L1702-L1710 (chrome/m156)
def_graphite_adapter_test!(
    SkRuntimeShader_TransformedCoords_Graphite,
    |reporter, context| {
        let recorder = context.make_recorder(None);
        let info = ImageInfo::new((12, 1), ColorType::RGBA8888, AlphaType::Premul, None);
        let surface = GraphiteSurface::render_target(&recorder, &info, Mipmapped::No, None, "");
        reporter_assert!(reporter, surface.is_some());
        let Some(surface) = surface else {
            return;
        };
        test_using_transformed_coords(
            reporter,
            &recorder,
            &mut GraphiteTestSurface {
                context,
                surface: &surface,
            },
        );
    }
);

// Port of: tests/SkRuntimeEffectTest.cpp#L1744-L1788 (chrome/m156)
def_test!(SkRuntimeShader_b500080194, |r| {
    const K_SKSL: &str = "half4 main(float2 xy) {\
          float4 v;\
          v.x += xy.x;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          (v = abs(v)).xyz;\
          return half4(v);\
        }";

    match RuntimeEffect::make_for_shader(K_SKSL, None) {
        Err(_) => {
            errorf!(r, "SkSL compile failed: SkSL compile failed");
        }
        Ok(effect) => {
            let shader = effect.make_shader(skia_rust_core::data::Data::new_empty(), &[], None);
            let mut paint = Paint::default();
            paint.set_shader(shader);
            let mut surface = surfaces::raster_n32_premul((64, 64)).expect("a raster surface");
            // This caused a crash before the patch.
            surface.canvas().draw_paint(&paint);
        }
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1849-L1866 (chrome/m156)
def_test!(SkRuntimeBlender_b466686344, |r| {
    // b/466686344: large loops in strict ES2 runtime effects must not cause execution
    // timeouts. Under fuzzer builds (SK_BUILD_FOR_FUZZER), kLoopTerminationLimit is lowered to
    // 256.
    const K_SKSL: &str = "half4 main(half4 src, half4 dst) {\
                half y = 1.0;\
                for (int c = 0; c < 1000; ++c) {\
                    y += 1.0;\
                }\
                return half4(y);\
            }";

    let effect = RuntimeEffect::make_for_blender(K_SKSL, None);
    // (`SK_BUILD_FOR_FUZZER` is not defined.)
    reporter_assert!(r, effect.is_ok());
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1868-L1913 (chrome/m156)
def_test!(SkRuntimeColorFilter_b466744542, |r| {
    // b/466744542: nested loops multiplying to excessive iteration counts must be rejected
    // under fuzzer builds to prevent timeouts in skruntimecolorfilter.
    let check_loop = |r: &mut Reporter, sksl: &str| {
        let result = RuntimeEffect::make_for_color_filter(sksl, None);
        // (`SK_BUILD_FOR_FUZZER` is not defined.)
        reporter_assert!(
            r,
            result.is_ok(),
            "{}",
            result.as_ref().err().cloned().unwrap_or_default()
        );
    };

    // 1. Nested loops: 20 * 20 = 400 iterations (> 256).
    check_loop(
        r,
        "half4 main(half4 color) {\
             half4 x = color;\
             for (int a = 0; a < 20; ++a) {\
                 for (int b = 0; b < 20; ++b) {\
                     x += half4(0.01);\
                 }\
             }\
             return x;\
         }",
    );

    // 2. Nested unrollable loops in ES3 mode: 20 * 20 = 400 iterations (> 256).
    // Color filters require #version 100 in non-fuzzer builds, so test ES3 mode with
    // MakeForShader.
    {
        let result = RuntimeEffect::make_for_shader(
            "#version 300\n\
             half4 main(float2 coords) {\
                 half4 x = half4(coords, 0, 1);\
                 for (int a = 0; a < 20; ++a) {\
                     for (int b = 0; b < 20; ++b) {\
                         x += half4(0.01);\
                     }\
                 }\
                 return x;\
             }",
            None,
        );
        reporter_assert!(
            r,
            result.is_ok(),
            "{}",
            result.as_ref().err().cloned().unwrap_or_default()
        );
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L759-L773 (chrome/m156)
fn verify_draw_obeys_capabilities(
    r: &mut Reporter,
    effect: &RuntimeEffect,
    surface: &mut Surface<'_>,
    paint: &Paint,
) {
    // We expect the draw to do something if-and-only-if expectSuccess is true:
    let expect_success = Capabilities::raster_backend().sksl_version() >= Version::K300;

    let k_green: u32 = 0xFF00_FF00;
    let k_red: u32 = 0xFF00_00FF;
    let expected = if expect_success { k_green } else { k_red };

    surface.canvas().clear(colors::RED);
    surface.canvas().draw_paint(paint);
    verify_2x2_surface_results(r, effect, surface, [expected; 4]);
}

// Port of: tests/SkRuntimeEffectTest.cpp#L1247-L1262 (chrome/m156)
def_test!(SkRuntimeBlendBuilderReuse, |r| {
    let k_source = "
        uniform half x;
        half4 main(half4 s, half4 d) { return half4(x); }
    ";

    let effect = RuntimeEffect::make_for_blender(k_source, None);
    reporter_assert!(r, effect.is_ok());
    let Ok(effect) = effect else {
        return;
    };

    // We should be able to construct multiple SkBlenders in a row without asserting.
    let mut b = RuntimeShaderBuilder::new(effect);
    let mut x = 0.0_f32;
    while x <= 2.0 {
        b.uniform("x").set_f32(&[x]);
        let _blender = b.make_blender();
        x += 2.0;
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1355-L1368 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)]
    SkRuntimeColorFilterSingleColor,
    |r| {
        // Test runtime colorfilters support filterColor4f().
        let effect =
            RuntimeEffect::make_for_color_filter("half4 main(half4 c) { return c*c; }", None);
        reporter_assert!(r, effect.is_ok());
        let Ok(effect) = effect else {
            return;
        };

        let cf = effect.make_color_filter(Data::new_empty(), &[]);
        reporter_assert!(r, cf.is_some());
        let Some(cf) = cf else {
            return;
        };

        let srgb = srgb_singleton();
        let c = cf.filter_color4f(Color4f::new(0.25, 0.5, 0.75, 1.0), Some(srgb), Some(srgb));
        reporter_assert!(r, c.r == 0.0625);
        reporter_assert!(r, c.g == 0.25);
        reporter_assert!(r, c.b == 0.5625);
        reporter_assert!(r, c.a == 1.0);
    }
);

// Port of: tests/SkRuntimeEffectTest.cpp#L1412-L1462 (chrome/m156)
def_test!(SkRuntimeColorFilterFlags, |r| {
    // Builds `shader` as a color filter and checks its `isAlphaUnchanged` against `expected`.
    let check = |r: &mut Reporter, shader: &str, expected: bool| {
        let effect = RuntimeEffect::make_for_color_filter(shader, None);
        reporter_assert!(r, effect.is_ok(), "{}", shader);
        let Ok(effect) = effect else {
            return;
        };
        let filter = effect.make_color_filter(Data::new_empty(), &[]);
        reporter_assert!(
            r,
            filter
                .as_ref()
                .is_some_and(|f| f.is_alpha_unchanged() == expected),
            "{}",
            shader
        );
    };
    let expect_alpha_unchanged = |r: &mut Reporter, shader: &str| check(r, shader, true);
    let expect_alpha_changed = |r: &mut Reporter, shader: &str| check(r, shader, false);

    // We expect these patterns to be detected as alpha-unchanged.
    expect_alpha_unchanged(r, "half4 main(half4 color) { return color; }");
    expect_alpha_unchanged(r, "half4 main(half4 color) { return color.aaaa; }");
    expect_alpha_unchanged(r, "half4 main(half4 color) { return color.bgra; }");
    expect_alpha_unchanged(r, "half4 main(half4 color) { return color.rraa; }");
    expect_alpha_unchanged(r, "half4 main(half4 color) { return color.010a; }");
    expect_alpha_unchanged(
        r,
        "half4 main(half4 color) { return half4(0, 0, 0, color.a); }",
    );
    expect_alpha_unchanged(
        r,
        "half4 main(half4 color) { return half4(half2(1), color.ba); }",
    );
    expect_alpha_unchanged(
        r,
        "half4 main(half4 color) { return half4(half2(1), half2(color.a)); }",
    );
    expect_alpha_unchanged(r, "half4 main(half4 color) { return half4(color.a); }");
    expect_alpha_unchanged(
        r,
        "half4 main(half4 color) { return half4(float4(color.baba)); }",
    );
    expect_alpha_unchanged(
        r,
        concat!(
            "half4 main(half4 color) { return color.r != color.g ? color :",
            "                                                                              color.000a; }"
        ),
    );
    expect_alpha_unchanged(
        r,
        concat!(
            "half4 main(half4 color) { return color.a == color.r ? color.rrra : ",
            "color.g == color.b ? color.ggga : ",
            "   color.bbba; }"
        ),
    );
    // Modifying the input color invalidates the check.
    expect_alpha_changed(r, "half4 main(half4 color) { color.a = 0; return color; }");

    // These swizzles don't end in alpha.
    expect_alpha_changed(r, "half4 main(half4 color) { return color.argb; }");
    expect_alpha_changed(r, "half4 main(half4 color) { return color.rrrr; }");

    // This compound constructor doesn't end in alpha.
    expect_alpha_changed(
        r,
        "half4 main(half4 color) { return half4(1, 1, 1, color.r); }",
    );

    // This splat constructor doesn't use alpha.
    expect_alpha_changed(r, "half4 main(half4 color) { return half4(color.r); }");

    // These ternaries don't return alpha on both sides
    expect_alpha_changed(
        r,
        "half4 main(half4 color) { return color.a > 0 ? half4(0) : color; }",
    );
    expect_alpha_changed(
        r,
        "half4 main(half4 color) { return color.g < 1 ? color.bgra : color.abgr; }",
    );
    expect_alpha_changed(
        r,
        "half4 main(half4 color) { return color.b > 0.5 ? half4(0) : half4(1); }",
    );

    // Performing arithmetic on the input causes it to report as "alpha changed" even if the
    // arithmetic is a no-op; we aren't smart enough to see through it.
    expect_alpha_changed(
        r,
        "half4 main(half4 color) { return color + half4(1,1,1,0); }",
    );
    expect_alpha_changed(
        r,
        "half4 main(half4 color) { return color + half4(0,0,0,4); }",
    );

    // All exit paths are checked.
    expect_alpha_changed(
        r,
        concat!(
            "half4 main(half4 color) { ",
            "    if (color.r > 0.5) { return color; }",
            "    return half4(0);",
            "}"
        ),
    );
    expect_alpha_changed(
        r,
        concat!(
            "half4 main(half4 color) { ",
            "    if (color.r > 0.5) { return half4(0); }",
            "    return color;",
            "}"
        ),
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1976-L1990 (chrome/m156)
def_test!(SkRuntimeColorFilter_b520831887, |r| {
    // b/520831887: loops with 16-bit integer induction variables (e.g. mediump int)
    // must not trigger an assertion failure in GetLoopUnrollInfo.
    let k_sksl = concat!(
        "half4 main(half4 color) {",
        "    for (mediump int b = 2; b < 4; ++b) {}",
        "    return color;",
        "}"
    );

    let effect = RuntimeEffect::make_for_color_filter(k_sksl, None);
    reporter_assert!(
        r,
        effect.is_ok(),
        "{}",
        effect.as_ref().err().cloned().unwrap_or_default()
    );
    if let Ok(effect) = effect {
        let cf = effect.make_color_filter(Data::new_empty(), &[]);
        reporter_assert!(r, cf.is_some());
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L858-L872 (chrome/m156)
def_test!(SkRuntimeEffectObeysCapabilities_CPU, |r| {
    let mut surface = make_surface((2, 2));
    test_runtime_effect_obeys_capabilities(r, &mut surface);
});

// Port of: tests/SkRuntimeEffectTest.cpp#L775-L857 (chrome/m156)
fn test_runtime_effect_obeys_capabilities(r: &mut Reporter, surface: &mut Surface<'_>) {
    // This test creates shaders and blenders that target `#version 300`. If a user validates an
    // effect like this against a particular device, and later draws that effect to a device with
    // insufficient capabilities -- we want to fail gracefully (drop the draw entirely).
    // If the capabilities indicate that the effect is supported, we expect it to work.
    //
    // We test two different scenarios here:
    // 1) An effect flagged as #version 300, but actually compatible with #version 100.
    // 2) An effect flagged as #version 300, and using features not available in ES2.
    //
    // We expect both cases to fail cleanly on ES2-only devices -- nothing should be drawn, and
    // there should be no asserts or driver shader-compilation errors.
    //
    // In all tests, we first clear the canvas to RED, then draw an effect that (if it renders)
    // will fill the canvas with GREEN. We check that the final colors match our expectations,
    // based on the device capabilities.

    // Effect that would actually work on CPU/ES2, but should still fail on those devices:
    {
        let effect = RuntimeEffect::make_for_shader(
            "
            #version 300
            half4 main(float2 xy) { return half4(0, 1, 0, 1); }
        ",
            None,
        );
        let Ok(effect) = effect else {
            errorf!(r, "effect did not compile");
            return;
        };
        let mut paint = Paint::default();
        let shader = effect.make_shader(Data::new_empty(), &[], None);
        reporter_assert!(r, shader.is_some());
        paint.set_shader(shader);
        reporter_assert!(r, paint.shader().is_some());
        verify_draw_obeys_capabilities(r, &effect, surface, &paint);
    }

    // Effect that won't work on CPU/ES2 at all, and should fail gracefully on those devices.
    // We choose to use bit-pun intrinsics because SkSL doesn't automatically inject an extension
    // to enable them (like it does for derivatives). We pass a non-literal value so that SkSL's
    // constant folding doesn't elide them entirely before the driver sees the shader.
    {
        let effect = RuntimeEffect::make_for_shader(
            "
            #version 300
            half4 main(float2 xy) {
                half4 result = half4(0, 1, 0, 1);
                result.g = intBitsToFloat(floatBitsToInt(result.g));
                return result;
            }
        ",
            None,
        );
        let Ok(effect) = effect else {
            errorf!(r, "effect did not compile");
            return;
        };
        let mut paint = Paint::default();
        let shader = effect.make_shader(Data::new_empty(), &[], None);
        reporter_assert!(r, shader.is_some());
        paint.set_shader(shader);
        reporter_assert!(r, paint.shader().is_some());
        verify_draw_obeys_capabilities(r, &effect, surface, &paint);
    }

    //
    // As above, but with a blender
    //

    {
        let effect = RuntimeEffect::make_for_blender(
            "
            #version 300
            half4 main(half4 src, half4 dst) { return half4(0, 1, 0, 1); }
        ",
            None,
        );
        let Ok(effect) = effect else {
            errorf!(r, "effect did not compile");
            return;
        };
        let mut paint = Paint::default();
        let blender = effect.make_blender(Data::new_empty(), &[]);
        reporter_assert!(r, blender.is_some());
        paint.set_blender(blender);
        reporter_assert!(r, paint.blender().is_some());
        verify_draw_obeys_capabilities(r, &effect, surface, &paint);
    }

    {
        let effect = RuntimeEffect::make_for_blender(
            "
            #version 300
            half4 main(half4 src, half4 dst) {
                half4 result = half4(0, 1, 0, 1);
                result.g = intBitsToFloat(floatBitsToInt(result.g));
                return result;
            }
        ",
            None,
        );
        let Ok(effect) = effect else {
            errorf!(r, "effect did not compile");
            return;
        };
        let mut paint = Paint::default();
        let blender = effect.make_blender(Data::new_empty(), &[]);
        reporter_assert!(r, blender.is_some());
        paint.set_blender(blender);
        reporter_assert!(r, paint.blender().is_some());
        verify_draw_obeys_capabilities(r, &effect, surface, &paint);
    }
}

// Port of: tests/SkRuntimeEffectTest.cpp#L1879-L1903 (chrome/m156)
def_test!(SkRuntimeShader_b416061512, |r| {
    let k_sksl = concat!(
        "half4 main(half4 s,half4){",
        "int x = int(s.x);",
        "return half4(half(x - -2147483648));",
        "}"
    );

    let effect = RuntimeEffect::make_for_blender(k_sksl, None);
    match effect {
        Err(err) => {
            errorf!(r, "SkSL compile failed: {}", err);
        }
        Ok(effect) => {
            let blender = effect.make_blender(Data::new_empty(), &[]);
            reporter_assert!(r, blender.is_some());
            let Some(blender) = blender else {
                return;
            };
            let mut paint = Paint::default();
            paint.set_color(Color::RED);
            paint.set_blender(blender);

            let info = ImageInfo::new((4, 4), ColorType::N32, AlphaType::Premul, None);
            let s = surfaces::raster(&info, None, None);
            reporter_assert!(r, s.is_some());
            if let Some(mut s) = s {
                // We should make sure this doesn't crash
                s.canvas().draw_paint(&paint);
            }
        }
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1846-L1874 (chrome/m156)
def_test!(SkRuntimeShader_b507643404, |r| {
    let k_sksl = concat!(
        "half4 blend_src_over(half4,half4 dst){",
        "float a;return(a)/dst;",
        "}",
        "half4 main(half4 src,half4){",
        "return blend_src_over(src,half4(0));",
        "}"
    );

    // This effect compiles when we aren't optimizing/inlining, but fails when we are.
    let effect = RuntimeEffect::make_for_blender(k_sksl, None);
    match effect {
        Err(err) => {
            errorf!(r, "SkSL compile failed: {}", err);
        }
        Ok(effect) => {
            let blender = effect.make_blender(Data::new_empty(), &[]);
            reporter_assert!(r, blender.is_some());
            let Some(blender) = blender else {
                return;
            };
            let mut paint = Paint::default();
            paint.set_color(Color::RED);
            paint.set_blender(blender);

            let info = ImageInfo::new((4, 4), ColorType::N32, AlphaType::Premul, None);
            let s = surfaces::raster(&info, None, None);
            reporter_assert!(r, s.is_some());
            if let Some(mut s) = s {
                // We should make sure this doesn't crash
                s.canvas().draw_paint(&paint);
            }
        }
    }
});

/// `TestBlend`: a 2x2 surface drawn with runtime blenders.
// Port of: tests/SkRuntimeEffectTest.cpp#L546-L613 (chrome/m156)
struct TestBlend {
    surface: Surface<'static>,
    builder: Option<RuntimeShaderBuilder>,
}

impl TestBlend {
    fn new() -> Self {
        TestBlend {
            surface: make_surface((2, 2)),
            builder: None,
        }
    }

    fn build(&mut self, r: &mut Reporter, src: &str, allow_private_access: bool) {
        let mut options = Options::default();
        if allow_private_access {
            runtime_effect_priv::allow_private_access(&mut options);
        }
        match RuntimeEffect::make_for_blender(src, Some(&options)) {
            Ok(effect) => self.builder = Some(RuntimeShaderBuilder::new(effect)),
            Err(error_text) => {
                errorf!(r, "Effect didn't compile: {}", error_text);
            }
        }
    }

    fn builder(&mut self) -> &mut RuntimeShaderBuilder {
        self.builder.as_mut().expect("a built effect")
    }

    fn uniform_f32(&mut self, name: &str, val: &[f32]) {
        self.builder().uniform(name).set_f32(val);
    }

    fn uniform_i32(&mut self, name: &str, val: &[i32]) {
        self.builder().uniform(name).set_i32(val);
    }

    fn child_null(&mut self, name: &str) {
        self.builder().child(name).assign_null();
    }

    fn child(&mut self, name: &str, child: ChildPtr) {
        self.builder().child(name).assign(child);
    }

    fn test(
        &mut self,
        r: &mut Reporter,
        expected: [u32; 4],
        pre_test_callback: Option<PreTestFn<'_>>,
    ) {
        let Some(blender) = self.builder().make_blender() else {
            errorf!(r, "Effect didn't produce a blender");
            return;
        };

        let mut paint = Paint::default();
        paint.set_blender(blender);
        paint.set_color(Color::GRAY);

        paint_canvas(self.surface.canvas(), &mut paint, pre_test_callback);

        let effect = self.builder().effect().clone();
        verify_2x2_surface_results(r, &effect, &mut self.surface, expected);
    }

    fn test_uniform(
        &mut self,
        r: &mut Reporter,
        expected: u32,
        pre_test_callback: Option<PreTestFn<'_>>,
    ) {
        self.test(r, [expected; 4], pre_test_callback);
    }
}

/// Fills `surface` with the RGBW shader, as `rgbwPaint` does.
fn draw_rgbw(surface: &mut Surface<'_>) {
    let mut paint = Paint::default();
    paint.set_shader(make_rgbw_shader());
    paint.set_blend_mode(BlendMode::Src);
    surface.canvas().draw_paint(&paint);
}

// Port of: tests/SkRuntimeEffectTest.cpp#L1076-L1215 (chrome/m156)
fn test_runtime_effect_blenders(r: &mut Reporter) {
    let mut effect = TestBlend::new();

    // Use of a simple uniform. (Draw twice with two values to ensure it's updated).
    effect.build(
        r,
        "uniform float4 gColor; half4 main(half4 s, half4 d) { return half4(gColor); }",
        false,
    );
    effect.uniform_f32("gColor", &[0.0, 0.25, 0.75, 1.0]);
    effect.test_uniform(r, 0xFFBF_4000, None);
    effect.uniform_f32("gColor", &[1.0, 0.0, 0.0, 0.498]);
    effect.test_uniform(r, 0x7F00_00FF, None); // We don't clamp here either

    // Same, with integer uniforms
    effect.build(
        r,
        "uniform int4 gColor;\
         half4 main(half4 s, half4 d) { return half4(gColor) / 255.0; }",
        false,
    );
    effect.uniform_i32("gColor", &[0x00, 0x40, 0xBF, 0xFF]);
    effect.test_uniform(r, 0xFFBF_4000, None);
    effect.uniform_i32("gColor", &[0xFF, 0x00, 0x00, 0x7F]);
    effect.test_uniform(r, 0x7F00_00FF, None); // We don't clamp here either

    // Verify that mutating the source and destination colors is allowed
    effect.build(
        r,
        "half4 main(half4 s, half4 d) { s += d; d += s; return half4(1); }",
        false,
    );
    effect.test_uniform(r, 0xFFFF_FFFF, None);

    // Verify that we can write out the source color (ignoring the dest color)
    // This is equivalent to the kSrc blend mode.
    effect.build(r, "half4 main(half4 s, half4 d) { return s; }", false);
    effect.test_uniform(r, 0xFF88_8888, None);

    // Fill the destination with a variety of colors (using the RGBW shader)
    draw_rgbw(&mut effect.surface);

    // Verify that we can read back the dest color exactly as-is (ignoring the source color)
    // This is equivalent to the kDst blend mode.
    effect.build(r, "half4 main(half4 s, half4 d) { return d; }", false);
    effect.test(
        r,
        [0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000, 0xFFFF_FFFF],
        None,
    );

    // Verify that we can invert the destination color (including the alpha channel).
    // The expected outputs are the exact inverse of the previous test.
    effect.build(
        r,
        "half4 main(half4 s, half4 d) { return half4(1) - d; }",
        false,
    );
    effect.test(
        r,
        [0x00FF_FF00, 0x00FF_00FF, 0x0000_FFFF, 0x0000_0000],
        None,
    );

    // Verify that color values are clamped to 0 and 1.
    effect.build(
        r,
        "half4 main(half4 s, half4 d) { return half4(-1); }",
        false,
    );
    effect.test_uniform(r, 0x0000_0000, None);
    effect.build(
        r,
        "half4 main(half4 s, half4 d) { return half4(2); }",
        false,
    );
    effect.test_uniform(r, 0xFFFF_FFFF, None);

    //
    // Sampling children
    //

    // Sampling a null shader should return transparent black.
    effect.build(
        r,
        "uniform shader child;\
         half4 main(half4 s, half4 d) { return child.eval(s.rg); }",
        false,
    );
    effect.child_null("child");
    effect.test_uniform(
        r,
        0x0000_0000,
        Some(&|_: &Canvas, paint: &mut Paint| {
            paint.set_color4f(Color4f::new(1.0, 1.0, 0.0, 1.0), None::<&ColorSpace>);
        }),
    );

    effect.build(
        r,
        "uniform colorFilter child;\
         half4 main(half4 s, half4 d) { return child.eval(s); }",
        false,
    );
    effect.child_null("child");
    effect.test_uniform(
        r,
        0xFF00_FFFF,
        Some(&|_: &Canvas, paint: &mut Paint| {
            paint.set_color4f(Color4f::new(1.0, 1.0, 0.0, 1.0), None::<&ColorSpace>);
        }),
    );

    // Sampling a null blender should do a src-over blend. Draw 50% black over RGBW to verify this.
    draw_rgbw(&mut effect.surface);
    effect.build(
        r,
        "uniform blender child;\
         half4 main(half4 s, half4 d) { return child.eval(s, d); }",
        false,
    );
    effect.child_null("child");
    effect.test(
        r,
        [0xFF00_0080, 0xFF00_8000, 0xFF80_0000, 0xFF80_8080],
        Some(&|_: &Canvas, paint: &mut Paint| {
            paint.set_color4f(Color4f::new(0.0, 0.0, 0.0, 0.497), None::<&ColorSpace>);
        }),
    );

    // Sampling a shader at various coordinates
    effect.build(
        r,
        "uniform shader child;\
         uniform half2 pos;\
         half4 main(half4 s, half4 d) { return child.eval(pos); }",
        false,
    );
    effect.child("child", ChildPtr::from(make_rgbw_shader()));
    effect.uniform_f32("pos", &[0.5, 0.5]);
    effect.test_uniform(r, 0xFF00_00FF, None);

    effect.uniform_f32("pos", &[1.5, 0.5]);
    effect.test_uniform(r, 0xFF00_FF00, None);

    effect.uniform_f32("pos", &[0.5, 1.5]);
    effect.test_uniform(r, 0xFFFF_0000, None);

    effect.uniform_f32("pos", &[1.5, 1.5]);
    effect.test_uniform(r, 0xFFFF_FFFF, None);

    // Sampling a shader as above, but via a helper function
    effect.build(
        r,
        "uniform shader child;\
         uniform half2 pos;\
         half4 eval_at_pos(shader x) { return x.eval(pos); }\
         half4 main(half4 s, half4 d) { return eval_at_pos(child); }",
        true,
    );
    effect.child("child", ChildPtr::from(make_rgbw_shader()));
    effect.uniform_f32("pos", &[0.5, 0.5]);
    effect.test_uniform(r, 0xFF00_00FF, None);

    effect.uniform_f32("pos", &[1.5, 0.5]);
    effect.test_uniform(r, 0xFF00_FF00, None);

    effect.uniform_f32("pos", &[0.5, 1.5]);
    effect.test_uniform(r, 0xFFFF_0000, None);

    effect.uniform_f32("pos", &[1.5, 1.5]);
    effect.test_uniform(r, 0xFFFF_FFFF, None);

    // Sampling a color filter
    effect.build(
        r,
        "uniform colorFilter child;\
         half4 main(half4 s, half4 d) { return child.eval(half4(1)); }",
        false,
    );
    let blue_filter = color_filters::blend_color(Color::new(0xFF01_2345), BlendMode::Src);
    effect.child("child", blue_filter.map_or(ChildPtr::Empty, ChildPtr::from));
    effect.test_uniform(r, 0xFF45_2301, None);

    // Sampling a built-in blender
    draw_rgbw(&mut effect.surface);
    effect.build(
        r,
        "uniform blender child;\
         half4 main(half4 s, half4 d) { return child.eval(s, d); }",
        false,
    );
    effect.child("child", ChildPtr::from(Blender::mode(BlendMode::Plus)));
    effect.test(
        r,
        [0xFF45_23FF, 0xFF45_FF01, 0xFFFF_2301, 0xFFFF_FFFF],
        Some(&|_: &Canvas, paint: &mut Paint| {
            paint.set_color(Color::new(0xFF01_2345));
        }),
    );

    // Sampling a runtime-effect blender
    draw_rgbw(&mut effect.surface);
    effect.build(
        r,
        "uniform blender child;\
         half4 main(half4 s, half4 d) { return child.eval(s, d); }",
        false,
    );
    let arithmetic = effects_blenders::arithmetic(0.0, 1.0, 1.0, 0.0, false);
    effect.child("child", arithmetic.map_or(ChildPtr::Empty, ChildPtr::from));
    effect.test(
        r,
        [0xFF45_23FF, 0xFF45_FF01, 0xFFFF_2301, 0xFFFF_FFFF],
        Some(&|_: &Canvas, paint: &mut Paint| {
            paint.set_color(Color::new(0xFF01_2345));
        }),
    );
}

// Port of: tests/SkRuntimeEffectTest.cpp#L1216-L1218 (chrome/m156)
def_test!(SkRuntimeEffect_Blender_CPU, |r| {
    test_runtime_effect_blenders(r);
});

// Port of: tests/SkRuntimeEffectTest.cpp#L925-L970 (chrome/m156)
def_test!(SkRuntimeEffectTraceShader, |r| {
    for image_size in [2, 80] {
        let mut effect = TestEffect::with_size((image_size, image_size));
        effect.build(
            r,
            r"
            half4 main(float2 p) {
                float2 val = p - 0.5;
                return val.0y01;
            }
        ",
        );
        let center = image_size / 2;
        let dump = effect.trace(r, IPoint { x: center, y: 1 });
        const SK_RP_SLOT_DUMP: &str = r"$0 = p (float2 : slot 1/2, L0)
$1 = p (float2 : slot 2/2, L0)
$2 = [main].result (float4 : slot 1/4, L0)
$3 = [main].result (float4 : slot 2/4, L0)
$4 = [main].result (float4 : slot 3/4, L0)
$5 = [main].result (float4 : slot 4/4, L0)
$6 = val (float2 : slot 1/2, L0)
$7 = val (float2 : slot 2/2, L0)
F0 = half4 main(float2 p)
";
        let expected_trace = format!(
            r"
enter half4 main(float2 p)
  p.x = {center}.5
  p.y = 1.5
  scope +1
   line 3
   val.x = {center}
   val.y = 1
   line 4
   [main].result.x = 0
   [main].result.y = 1
   [main].result.z = 0
   [main].result.w = 1
  scope -1
exit half4 main(float2 p)
"
        );
        reporter_assert!(
            r,
            dump.starts_with(SK_RP_SLOT_DUMP) && dump.ends_with(&expected_trace),
            "Trace does not match expectation for {}x{}:\n{}\n",
            image_size,
            image_size,
            dump
        );
    }
});

// Port of: tests/SkRuntimeEffectTest.cpp#L972-L1030 (chrome/m156)
def_test!(SkRuntimeEffectTracesAreUnoptimized, |r| {
    let mut effect = TestEffect::new();

    effect.build(
        r,
        r"
        int globalUnreferencedVar = 7;
        half inlinableFunction() {
            return 1;
        }
        half4 main(float2 p) {
            if (true) {
                int localUnreferencedVar = 7;
            }
            return inlinableFunction().xxxx;
        }
    ",
    );
    let dump = effect.trace(r, IPoint { x: 1, y: 1 });
    const SK_RP_SLOT_DUMP: &str = r"$0 = p (float2 : slot 1/2, L0)
$1 = p (float2 : slot 2/2, L0)
$2 = globalUnreferencedVar (int, L0)
$3 = [main].result (float4 : slot 1/4, L0)
$4 = [main].result (float4 : slot 2/4, L0)
$5 = [main].result (float4 : slot 3/4, L0)
$6 = [main].result (float4 : slot 4/4, L0)
$7 = localUnreferencedVar (int, L0)
$8 = [inlinableFunction].result (float, L0)
F0 = half4 main(float2 p)
F1 = half inlinableFunction()
";
    const EXPECTED_TRACE: &str = r"
globalUnreferencedVar = 7
enter half4 main(float2 p)
  p.x = 1.5
  p.y = 1.5
  scope +1
   line 7
   scope +1
    line 8
    localUnreferencedVar = 7
   scope -1
   line 10
   enter half inlinableFunction()
     scope +1
      line 4
      [inlinableFunction].result = 1
     scope -1
   exit half inlinableFunction()
   [main].result.x = 1
   [main].result.y = 1
   [main].result.z = 1
   [main].result.w = 1
  scope -1
exit half4 main(float2 p)
";
    reporter_assert!(
        r,
        dump.starts_with(SK_RP_SLOT_DUMP) && dump.ends_with(EXPECTED_TRACE),
        "Trace output does not match expectation:\n{}\n",
        dump
    );
});

// Port of: tests/SkRuntimeEffectTest.cpp#L1032-L1074 (chrome/m156)
def_test!(SkRuntimeEffectTraceCodeThatCannotBeUnoptimized, |r| {
    let mut effect = TestEffect::new();

    effect.build(
        r,
        r"
        half4 main(float2 p) {
            int variableThatGetsOptimizedAway = 7;
            if (true) {
                return half4(1);
            }
            // This (unreachable) path doesn't return a value.
            // Without optimization, SkSL thinks this code doesn't return a value on every path.
        }
    ",
    );
    let dump = effect.trace(r, IPoint { x: 1, y: 1 });
    const SK_RP_SLOT_DUMP: &str = r"$0 = p (float2 : slot 1/2, L0)
$1 = p (float2 : slot 2/2, L0)
$2 = [main].result (float4 : slot 1/4, L0)
$3 = [main].result (float4 : slot 2/4, L0)
$4 = [main].result (float4 : slot 3/4, L0)
$5 = [main].result (float4 : slot 4/4, L0)
F0 = half4 main(float2 p)
";
    const EXPECTED_TRACE: &str = r"
enter half4 main(float2 p)
  p.x = 1.5
  p.y = 1.5
  scope +1
   scope +1
    line 5
    [main].result.x = 1
    [main].result.y = 1
    [main].result.z = 1
    [main].result.w = 1
   scope -1
  scope -1
exit half4 main(float2 p)
";
    reporter_assert!(
        r,
        dump.starts_with(SK_RP_SLOT_DUMP) && dump.ends_with(EXPECTED_TRACE),
        "Trace output does not match expectation:\n{}\n",
        dump
    );
});
