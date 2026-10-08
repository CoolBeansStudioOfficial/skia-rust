// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/runtimeshader.cpp (chrome/m156)

//! Runtime shader GMs.
//!
//! Not ported yet (left `todo` in the manifest): `ThresholdRT`, `UnsharpRT`, `ColorCubeRT`,
//! `ColorCubeColorFilterRT` and `local_matrix_shader_rt` load images (codecs are not ported);
//! `ClipSuperRRect` is not in the manifest.

// SkIntToScalar of small values; the ported lambdas keep the C++ declaration order.
#![allow(clippy::cast_precision_loss, clippy::items_after_statements)]

use crate::prelude::*;
use crate::tool_utils::make_surface;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::colors;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::runtime_effect_priv;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

// Port of: gm/runtimeshader.cpp#L23-L28 (chrome/m156)
const K_BENCH_RT_FLAG: u32 = 0x2;
const K_ANIMATE_RT_FLAG: u32 = 0x1;

/// `RuntimeShaderGM`: the shared part of the GMs that draw one runtime shader.
// Port of: gm/runtimeshader.cpp#L30-L65 (chrome/m156)
struct RuntimeShaderGm {
    name: &'static str,
    size: ISize,
    #[allow(dead_code)] // the flags only matter to benches and animation
    flags: u32,
    sksl: &'static str,
    effect: Option<RuntimeEffect>,
    /// `fSecs`: the GM is not animated, so it stays 0.
    secs: f32,
}

impl RuntimeShaderGm {
    fn new(name: &'static str, size: ISize, sksl: &'static str, flags: u32) -> Self {
        RuntimeShaderGm {
            name,
            size,
            flags,
            sksl,
            effect: None,
            secs: 0.0,
        }
    }

    // Port of: gm/runtimeshader.cpp#L37-L45 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        match RuntimeEffect::make_for_shader(self.sksl, None) {
            Ok(effect) => self.effect = Some(effect),
            Err(error) => eprintln!("RuntimeShader error: {error}"),
        }
    }

    fn builder(&self) -> RuntimeShaderBuilder {
        RuntimeShaderBuilder::new(self.effect.clone().expect("the effect compiled"))
    }
}

/// `sizeof(SkColor4f)` bytes of a color.
fn color4f_bytes(c: Color4f) -> [f32; 4] {
    c.as_array()
}

// Port of: gm/runtimeshader.cpp#L67-L90 (chrome/m156)
struct SimpleRt(RuntimeShaderGm);

impl SimpleRt {
    fn new() -> Self {
        SimpleRt(RuntimeShaderGm::new(
            "runtime_shader",
            ISize::new(512, 256),
            r"
        uniform half4 gColor;

        half4 main(float2 p) {
            return half4(p*(1.0/255), gColor.b, 1);
        }
    ",
            K_BENCH_RT_FLAG,
        ))
    }
}

impl GM for SimpleRt {
    fn name(&self) -> String {
        self.0.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.0.size
    }

    fn on_once_before_draw(&mut self) {
        self.0.on_once_before_draw();
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut builder = self.0.builder();

        let mut local_m = Matrix::new_identity();
        local_m.set_rotate(90.0, Some((128.0, 128.0).into()));
        builder
            .uniform("gColor")
            .set_f32(&color4f_bytes(Color4f::new(1.0, 0.0, 0.0, 1.0)));

        let mut p = Paint::default();
        p.set_shader(builder.make_shader(&local_m));
        canvas.draw_rect(Rect::new(0.0, 0.0, 256.0, 256.0), &p);
    }
}
crate::def_gm!(SimpleRT, SimpleRt::new());

// Port of: gm/runtimeshader.cpp#L172-L210 (chrome/m156)
struct SpiralRt(RuntimeShaderGm);

impl SpiralRt {
    fn new() -> Self {
        SpiralRt(RuntimeShaderGm::new(
            "spiral_rt",
            ISize::new(512, 512),
            r"
        uniform float rad_scale;
        uniform float2 in_center;
        layout(color) uniform float4 in_colors0;
        layout(color) uniform float4 in_colors1;

        half4 main(float2 p) {
            float2 pp = p - in_center;
            float radius = length(pp);
            radius = sqrt(radius);
            float angle = atan(pp.y / pp.x);
            float t = (angle + 3.1415926/2) / (3.1415926);
            t += radius * rad_scale;
            t = fract(t);
            return in_colors0 * (1-t) + in_colors1 * t;
        }
    ",
            K_ANIMATE_RT_FLAG | K_BENCH_RT_FLAG,
        ))
    }
}

impl GM for SpiralRt {
    fn name(&self) -> String {
        self.0.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.0.size
    }

    fn on_once_before_draw(&mut self) {
        self.0.on_once_before_draw();
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut builder = self.0.builder();

        builder
            .uniform("rad_scale")
            .set_f32(&[(self.0.secs * 0.5 + 2.0).sin() / 5.0]);
        builder.uniform("in_center").set_f32(&[256.0, 256.0]);
        builder
            .uniform("in_colors0")
            .set_f32(&color4f_bytes(colors::RED));
        builder
            .uniform("in_colors1")
            .set_f32(&color4f_bytes(colors::GREEN));

        let mut paint = Paint::default();
        paint.set_shader(builder.make_shader(None));
        canvas.draw_rect(Rect::new(0.0, 0.0, 512.0, 512.0), &paint);
    }
}
crate::def_gm!(SpiralRT, SpiralRt::new());

// Port of: gm/runtimeshader.cpp#L450-L500 (chrome/m156)
struct LinearGradientRt(RuntimeShaderGm);

impl LinearGradientRt {
    fn new() -> Self {
        LinearGradientRt(RuntimeShaderGm::new(
            "linear_gradient_rt",
            ISize::new(256 + 10, 128 + 15),
            r"
        layout(color) uniform vec4 in_colors0;
        layout(color) uniform vec4 in_colors1;

        vec4 main(vec2 p) {
            float t = p.x / 256;
            if (p.y < 32) {
                return mix(in_colors0, in_colors1, t);
            } else {
                vec3 linColor0 = toLinearSrgb(in_colors0.rgb);
                vec3 linColor1 = toLinearSrgb(in_colors1.rgb);
                vec3 linColor = mix(linColor0, linColor1, t);
                return fromLinearSrgb(linColor).rgb1;
            }
        }
    ",
            0,
        ))
    }
}

impl GM for LinearGradientRt {
    fn name(&self) -> String {
        self.0.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.0.size
    }

    fn on_once_before_draw(&mut self) {
        self.0.on_once_before_draw();
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        // Colors chosen to use values other than 0 and 1 - so that it's obvious if the
        // conversion intrinsics are doing anything. (Most transfer functions map 0 -> 0 and
        // 1 -> 1).
        let mut builder = self.0.builder();
        builder
            .uniform("in_colors0")
            .set_f32(&color4f_bytes(Color4f::new(0.75, 0.25, 0.0, 1.0)));
        builder
            .uniform("in_colors1")
            .set_f32(&color4f_bytes(Color4f::new(0.0, 0.75, 0.25, 1.0)));
        let mut paint = Paint::default();
        paint.set_shader(builder.make_shader(None));

        canvas.save();
        canvas.clear(Color::WHITE);
        canvas.translate((5.0, 5.0));

        // We draw everything twice. First to a surface with no color management, where the
        // intrinsics should do nothing (eg, the top bar should look the same in the top and
        // bottom halves). Then to an sRGB surface, where they should produce linearly
        // interpolated gradients (the bottom half of the second bar should be brighter than the
        // top half).
        for cs in [None, Some(ColorSpace::new_srgb())] {
            let info = ImageInfo::new(
                (256, 64),
                skia_rust_core::color_type::ColorType::N32,
                skia_rust_core::alpha_type::AlphaType::Premul,
                cs,
            );
            let mut surface = make_surface(canvas, &info, None).expect("a surface");

            surface
                .canvas()
                .draw_rect(Rect::new(0.0, 0.0, 256.0, 64.0), &paint);
            canvas.draw_image(
                surface.image_snapshot().expect("a snapshot"),
                (0.0, 0.0),
                None,
            );
            canvas.translate((0.0, 64.0 + 5.0));
        }

        canvas.restore();
    }
}
crate::def_gm!(LinearGradientRT, LinearGradientRt::new());

// Port of: gm/runtimeshader.cpp#L502-L525 (chrome/m156)
crate::def_simple_gm!(child_sampling_rt, canvas, 256, 256, {
    const SCALE: &str = "uniform shader child;\
        half4 main(float2 xy) {\
            return child.eval(xy*0.1);\
        }";

    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(1.0);

    let mut surf = surfaces::raster_n32_premul((100, 100)).expect("a surface");
    surf.canvas().draw_line((0.0, 0.0), (100.0, 100.0), &p);
    let shader = surf.image_snapshot().expect("a snapshot").to_shader(
        None,
        SamplingOptions::from(FilterMode::Linear),
        None,
    );

    let mut builder = RuntimeShaderBuilder::new(
        RuntimeEffect::make_for_shader(SCALE, None).expect("the effect compiles"),
    );
    builder
        .child("child")
        .assign(shader.expect("an image shader"));
    p.set_shader(builder.make_shader(None));

    canvas.draw_paint(&p);
});

// Produces a hemispherical normal:
// Port of: gm/runtimeshader.cpp#L527-L541 (chrome/m156)
fn normal_map_shader() -> Shader {
    const SRC: &str = r"
        half4 main(vec2 p) {
            p = (p / 256) * 2 - 1;
            float p2 = dot(p, p);
            vec3 v = (p2 > 1) ? vec3(0, 0, 1) : vec3(p, sqrt(1 - p2));
            return (v * 0.5 + 0.5).xyz1;
        }
    ";
    let effect = RuntimeEffect::make_for_shader(SRC, None).expect("the effect compiles");
    effect
        .make_shader(Data::new_empty(), &[], None)
        .expect("a shader")
}

// Above, baked into an image:
// Port of: gm/runtimeshader.cpp#L543-L552 (chrome/m156)
fn normal_map_image() -> Image {
    let info = ImageInfo::new(
        (256, 256),
        skia_rust_core::color_type::ColorType::N32,
        skia_rust_core::alpha_type::AlphaType::Premul,
        None,
    );
    let mut surface = surfaces::raster(&info, None, None).expect("a surface");
    let mut p = Paint::default();
    p.set_shader(normal_map_shader());
    surface.canvas().draw_paint(&p);
    surface.image_snapshot().expect("a snapshot")
}

// Port of: gm/runtimeshader.cpp#L554-L556 (chrome/m156)
fn normal_map_image_shader() -> Shader {
    normal_map_image()
        .to_shader(None, SamplingOptions::from(FilterMode::Nearest), None)
        .expect("an image shader")
}

// Port of: gm/runtimeshader.cpp#L558-L560 (chrome/m156)
fn normal_map_raw_image_shader() -> Shader {
    normal_map_image()
        .to_raw_shader(None, SamplingOptions::from(FilterMode::Nearest), None)
        .expect("a raw image shader")
}

// Port of: gm/runtimeshader.cpp#L562-L577 (chrome/m156)
fn normal_map_unpremul_image() -> Image {
    let image = normal_map_image();
    let pm = image.peek_pixels().expect("pixels");
    let mut bmp = skia_rust_core::bitmap::Bitmap::new();
    bmp.alloc_pixels_info(
        &image
            .image_info()
            .with_alpha_type(skia_rust_core::alpha_type::AlphaType::Unpremul),
        None,
    );
    // Copy all pixels over, but set alpha to 0
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            bmp.set_addr32(x, y, pm.addr32(x, y) & 0x00FF_FFFF);
        }
    }
    bmp.as_image().expect("an image")
}

// Port of: gm/runtimeshader.cpp#L579-L581 (chrome/m156)
fn normal_map_unpremul_image_shader() -> Shader {
    normal_map_unpremul_image()
        .to_shader(None, SamplingOptions::from(FilterMode::Nearest), None)
        .expect("an image shader")
}

// Port of: gm/runtimeshader.cpp#L583-L585 (chrome/m156)
fn normal_map_raw_unpremul_image_shader() -> Shader {
    normal_map_unpremul_image()
        .to_raw_shader(None, SamplingOptions::from(FilterMode::Nearest), None)
        .expect("a raw image shader")
}

// Port of: gm/runtimeshader.cpp#L587-L601 (chrome/m156)
fn lit_shader(normals: Shader) -> Shader {
    // Simple N-dot-L against a fixed, directional light:
    const SRC: &str = r"
        uniform shader normals;
        half4 main(vec2 p) {
            vec3 n = normalize(normals.eval(p).xyz * 2 - 1);
            vec3 l = normalize(vec3(1, -1, 1));
            return saturate(dot(n, l)).xxx1;
        }
    ";
    let effect = RuntimeEffect::make_for_shader(SRC, None).expect("the effect compiles");
    effect
        .make_shader(Data::new_empty(), &[normals.into()], None)
        .expect("a shader")
}

// Port of: gm/runtimeshader.cpp#L603-L617 (chrome/m156)
fn lit_shader_linear(normals: Shader) -> Shader {
    // Simple N-dot-L against a fixed, directional light, done in linear space:
    const SRC: &str = r"
        uniform shader normals;
        half4 main(vec2 p) {
            vec3 n = normalize(normals.eval(p).xyz * 2 - 1);
            vec3 l = normalize(vec3(1, -1, 1));
            return fromLinearSrgb(saturate(dot(n, l)).xxx).xxx1;
        }
    ";
    let effect = RuntimeEffect::make_for_shader(SRC, None).expect("the effect compiles");
    effect
        .make_shader(Data::new_empty(), &[normals.into()], None)
        .expect("a shader")
}

// Port of: gm/runtimeshader.cpp#L619-L645 (chrome/m156)
crate::def_simple_gm!(paint_alpha_normals_rt, canvas, 512, 512, {
    // Various draws, with non-opaque paint alpha. This demonstrates several issues around how
    // paint alpha is applied differently on CPU (globally, after all shaders) and GPU (per
    // shader, inconsistently). See: skbug.com/40043035
    //
    // When this works, it will be a demo of applying paint alpha to fade out a complex effect.
    let draw_shader = |x: i32, y: i32, shader: Shader| {
        let mut p = Paint::default();
        p.set_alpha(164);
        p.set_shader(shader);

        canvas.save();
        canvas.translate((x as f32, y as f32));
        canvas.clip_rect(Rect::new(0.0, 0.0, 256.0, 256.0), None, None);
        canvas.draw_paint(&p);
        canvas.restore();
    };

    draw_shader(0, 0, normal_map_shader());
    draw_shader(0, 256, normal_map_image_shader());

    draw_shader(256, 0, lit_shader(normal_map_shader()));
    draw_shader(256, 256, lit_shader(normal_map_image_shader()));
});

/// The `draw_shader` lambda of `raw_image_shader_normals_rt` and `lit_shader_linear_rt`.
fn draw_shader_clipped(x: i32, y: i32, shader: Shader, canvas: &Canvas) {
    let mut p = Paint::default();
    p.set_shader(shader);

    canvas.save();
    canvas.translate((x as f32, y as f32));
    canvas.clip_rect(Rect::new(0.0, 0.0, 256.0, 256.0), None, None);
    canvas.draw_paint(&p);
    canvas.restore();
}

// Port of: gm/runtimeshader.cpp#L647-L698 (chrome/m156)
crate::def_simple_gm!(raw_image_shader_normals_rt, canvas, 768, 512, {
    // Demonstrates the utility of SkImage::makeRawShader, for non-color child shaders.

    // First, make an offscreen surface, so we can control the destination color space:
    let surf_info = ImageInfo::new(
        (512, 512),
        skia_rust_core::color_type::ColorType::N32,
        skia_rust_core::alpha_type::AlphaType::Premul,
        Some(ColorSpace::new_srgb().with_color_spin()),
    );
    let mut surface: Surface<'_> = make_surface(canvas, &surf_info, None).expect("a surface");

    let color_normals = normal_map_image_shader();
    let raw_normals = normal_map_raw_image_shader();

    // Draw our normal map as colors (will be color-rotated), and raw (untransformed)
    draw_shader_clipped(0, 0, color_normals.clone(), surface.canvas());
    draw_shader_clipped(0, 256, raw_normals.clone(), surface.canvas());

    // Now draw our lighting shader using the normal and raw versions of the normals as children.
    // The top image will have the normals rotated (incorrectly), so the lighting is very dark.
    draw_shader_clipped(256, 0, lit_shader(color_normals), surface.canvas());
    draw_shader_clipped(256, 256, lit_shader(raw_normals), surface.canvas());

    // Now draw the offscreen surface back to our original canvas. If we do this naively, the
    // image will be un-transformed back to the canvas' color space. That will have the effect of
    // undoing the color spin on the upper-left, and APPLYING a color-spin on the bottom left. To
    // preserve the intent of this GM (and make it draw consistently whether or not the original
    // surface has a color space attached), we reinterpret the offscreen image as being in sRGB:
    canvas.draw_image(
        surface
            .image_snapshot()
            .expect("a snapshot")
            .reinterpret_color_space(ColorSpace::new_srgb())
            .expect("a reinterpreted image"),
        (0.0, 0.0),
        None,
    );

    // Finally, to demonstrate that raw unpremul image shaders don't premul, draw lighting two
    // more times, with an unpremul normal map (containing ZERO in the alpha channel). THe top
    // will premultiply the normals, resulting in totally dark lighting. The bottom will retain
    // the RGB encoded normals, even with zero alpha:
    draw_shader_clipped(
        512,
        0,
        lit_shader(normal_map_unpremul_image_shader()),
        canvas,
    );
    draw_shader_clipped(
        512,
        256,
        lit_shader(normal_map_raw_unpremul_image_shader()),
        canvas,
    );
});

// Port of: gm/runtimeshader.cpp#L700-L726 (chrome/m156)
crate::def_simple_gm!(lit_shader_linear_rt, canvas, 512, 256, {
    // First, make an offscreen surface, so we can control the destination color space:
    let surf_info = ImageInfo::new(
        (512, 256),
        skia_rust_core::color_type::ColorType::N32,
        skia_rust_core::alpha_type::AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    );
    let mut surface: Surface<'_> = make_surface(canvas, &surf_info, None).expect("a surface");

    // We draw two lit spheres - one does math in the working space (so gamma-encoded). The
    // second works in linear space, then converts to sRGB. This produces (more accurate) sharp
    // falloff:
    draw_shader_clipped(0, 0, lit_shader(normal_map_shader()), surface.canvas());
    draw_shader_clipped(
        256,
        0,
        lit_shader_linear(normal_map_shader()),
        surface.canvas(),
    );

    // Now draw the offscreen surface back to our original canvas:
    canvas.draw_image(
        surface.image_snapshot().expect("a snapshot"),
        (0.0, 0.0),
        None,
    );
});

// Port of: gm/runtimeshader.cpp#L777-L822 (chrome/m156)
crate::def_simple_gm_can_fail!(deferred_shader_rt, canvas, error_msg, 150, 50, {
    // Skip this GM on recording devices. It actually works okay on serialize-8888, but pic-8888
    // does not. Ultimately, behavior on CPU is potentially strange (especially with SkRP),
    // because SkRP will build the shader more than once per draw.
    if canvas.image_info().color_type() == skia_rust_core::color_type::ColorType::Unknown {
        return DrawResult::Skip;
    }

    let _ = &error_msg;
    const SHADER: &str = r"
        uniform half4 color;
        half4 main(float2 p) { return color; }
    ";
    let effect = RuntimeEffect::make_for_shader(SHADER, None).expect("the effect compiles");

    // `mutable` lambda: the color rotates every time the uniforms are asked for.
    let color = std::sync::Mutex::new(colors::RED);
    let make_uniforms = move |_: &runtime_effect_priv::UniformsCallbackContext<'_>| -> Data {
        let mut color = color.lock().expect("the color");
        let result = Data::new_copy(
            &color
                .as_array()
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .collect::<Vec<u8>>(),
        );
        *color = Color4f::new(color.b, color.r, color.g, color.a);
        result
    };

    let shader = runtime_effect_priv::make_deferred_shader(
        &effect,
        std::sync::Arc::new(make_uniforms),
        &[],
        None,
    );
    assert!(shader.is_some());

    let mut paint = Paint::default();
    paint.set_shader(shader);

    for _ in 0..3 {
        canvas.draw_rect(Rect::new(0.0, 0.0, 50.0, 50.0), &paint);
        canvas.translate((50.0, 0.0));
    }

    DrawResult::Ok
});

// Port of: gm/runtimeshader.cpp#L909-L1000 (chrome/m156)
crate::def_simple_gm!(null_child_rt, canvas, 150, 100, {
    // Every swatch should evaluate to the same shade of purple.
    // Paint with a shader evaluating a null shader.
    // Point passed to eval() is ignored; transparent black is returned.
    {
        let rt_shader = RuntimeEffect::make_for_shader(
            "uniform shader s;\
             half4 main(float2 p) { return s.eval(p) + half4(0.5, 0, 0.5, 1); }",
            None,
        )
        .expect("the shader compiles");

        let mut paint = Paint::default();
        let children = [ChildPtr::Empty];
        paint.set_shader(rt_shader.make_shader(Data::new_empty(), &children, None));
        paint.set_color(Color::from_argb(0xFF, 0x00, 0xFF, 0x00)); // green (ignored)
        canvas.draw_rect(Rect::new(0.0, 0.0, 48.0, 48.0), &paint);
        canvas.translate((50.0, 0.0));
    }
    // Paint with a shader evaluating a null color filter.
    // Color passed to eval() is returned; paint color is ignored.
    {
        let rt_shader = RuntimeEffect::make_for_shader(
            "uniform colorFilter cf;\
             half4 main(float2 p) { return cf.eval(half4(0.5, 0, 0.5, 1)); }",
            None,
        )
        .expect("the shader compiles");

        let mut paint = Paint::default();
        let children = [ChildPtr::Empty];
        paint.set_shader(rt_shader.make_shader(Data::new_empty(), &children, None));
        paint.set_color(Color::from_argb(0xFF, 0x00, 0x00, 0xFF)); // green (does not contribute)
        canvas.draw_rect(Rect::new(0.0, 0.0, 48.0, 48.0), &paint);
        canvas.translate((50.0, 0.0));
    }
    // Paint with a shader evaluating a null blender.
    // Colors passed to eval() are blended via src-over; paint color is ignored.
    {
        let rt_shader = RuntimeEffect::make_for_shader(
            "uniform blender b;\
             half4 main(float2 p) { return b.eval(half4(0.5, 0, 0, 0.5), half4(0, 0, 1, 1)); }",
            None,
        )
        .expect("the shader compiles");

        let mut paint = Paint::default();
        let children = [ChildPtr::Empty];
        paint.set_shader(rt_shader.make_shader(Data::new_empty(), &children, None));
        paint.set_color(Color::from_argb(0xFF, 0x00, 0x00, 0xFF)); // green (does not contribute)
        canvas.draw_rect(Rect::new(0.0, 0.0, 48.0, 48.0), &paint);
        canvas.translate((50.0, 0.0));
    }

    canvas.translate((-150.0, 50.0));

    // Paint with a color filter evaluating a null shader.
    // Point passed to eval() is ignored; transparent black is returned.
    {
        let rt_filter = RuntimeEffect::make_for_color_filter(
            "uniform shader s;\
             half4 main(half4 c) { return s.eval(float2(0)) + half4(0.5, 0, 0.5, 1); }",
            None,
        )
        .expect("the color filter compiles");

        let mut paint = Paint::default();
        let children = [ChildPtr::Empty];
        paint.set_color_filter(rt_filter.make_color_filter(Data::new_empty(), &children));
        paint.set_color(Color::from_argb(0xFF, 0x00, 0xFF, 0x00)); // green (ignored)
        canvas.draw_rect(Rect::new(0.0, 0.0, 48.0, 48.0), &paint);
        canvas.translate((50.0, 0.0));
    }
    // Paint with a color filter evaluating a null color filter.
    // Color passed to eval() is returned; paint color is ignored.
    {
        let rt_filter = RuntimeEffect::make_for_color_filter(
            "uniform colorFilter cf;\
             half4 main(half4 c) { return cf.eval(half4(0.5, 0, 0.5, 1)); }",
            None,
        )
        .expect("the color filter compiles");

        let mut paint = Paint::default();
        let children = [ChildPtr::Empty];
        paint.set_color_filter(rt_filter.make_color_filter(Data::new_empty(), &children));
        paint.set_color(Color::from_argb(0xFF, 0x00, 0x00, 0xFF)); // green (does not contribute)
        canvas.draw_rect(Rect::new(0.0, 0.0, 48.0, 48.0), &paint);
        canvas.translate((50.0, 0.0));
    }
    // Paint with a color filter evaluating a null blender.
    // Colors passed to eval() are blended via src-over; paint color is ignored.
    {
        let rt_filter = RuntimeEffect::make_for_color_filter(
            "uniform blender b;\
             half4 main(half4 c) { return b.eval(half4(0.5, 0, 0, 0.5), half4(0, 0, 1, 1)); }",
            None,
        )
        .expect("the color filter compiles");

        let mut paint = Paint::default();
        let children = [ChildPtr::Empty];
        paint.set_color_filter(rt_filter.make_color_filter(Data::new_empty(), &children));
        paint.set_color(Color::from_argb(0xFF, 0x00, 0x00, 0xFF)); // green (does not contribute)
        canvas.draw_rect(Rect::new(0.0, 0.0, 48.0, 48.0), &paint);
        canvas.translate((50.0, 0.0));
    }
});

// Port of: gm/runtimeshader.cpp#L1057-L1062 (chrome/m156)
fn paint_color_shader() -> Option<Shader> {
    let mut bmp = Bitmap::new();
    bmp.alloc_pixels_info(
        &ImageInfo::new((1, 1), ColorType::Alpha8, AlphaType::Premul, None),
        None,
    );
    bmp.erase_color(Color::WHITE);
    bmp.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        FilterMode::Nearest,
        None,
    )
}

// Port of: gm/runtimeshader.cpp#L1064-L1133 (chrome/m156)
crate::def_simple_gm_can_fail!(alpha_image_shader_rt, canvas, error_msg, 350, 50, {
    // (Skia skips recording backends (DDL) here. The harness always draws to a surface, so the
    // check is not needed.)
    let _ = &error_msg;

    // Skia typically applies the paint color (or input color, for more complex GPU-FP trees)
    // to alpha-only images. This is useful in trivial cases, but surprising and inconsistent in
    // more complex situations, especially when using SkSL.
    //
    // This GM checks that we suppress the paint-color tinting from SkSL, and always get {0,0,0,a}.
    let checkerboard = crate::tool_utils::create_checkerboard_shader(Color::BLACK, Color::WHITE, 4);
    let paint_shader = paint_color_shader();
    let children = [paint_shader.clone().map_or(ChildPtr::Empty, ChildPtr::from)];

    let mut paint = Paint::default();
    paint.set_color4f(Color4f::new(0.5, 0.0, 0.5, 1.0), None::<&ColorSpace>);

    let rect = |canvas: &Canvas, paint: &Paint| {
        canvas.draw_rect(Rect::new(0.0, 0.0, 48.0, 48.0), paint);
        canvas.translate((50.0, 0.0));
    };

    // Two simple cases: just paint color, then the "paint color" shader.
    // These should both be PURPLE
    rect(canvas, &paint);

    paint.set_shader(paint_shader.clone());
    rect(canvas, &paint);

    // All remaining cases should be BLACK

    // Shader that evaluates the "paint color" shader.
    // For color-filter and blender, we test them with and without an actual SkShader on the paint.
    // These should all be BLACK
    let shader_effect = RuntimeEffect::make_for_shader(
        "uniform shader s;\
         half4 main(float2 p) { return s.eval(p); }",
        None,
    )
    .expect("the shader compiles");
    paint.set_shader(shader_effect.make_shader(Data::new_empty(), &children, None));
    rect(canvas, &paint);

    // Color-filter that evaluates the "paint color" shader, with and without a shader on the paint
    paint.set_shader(None::<Shader>);
    let cf_effect = RuntimeEffect::make_for_color_filter(
        "uniform shader s;\
         half4 main(half4 color) { return s.eval(float2(0)); }",
        None,
    )
    .expect("the color filter compiles");
    paint.set_color_filter(cf_effect.make_color_filter(Data::new_empty(), &children));
    rect(canvas, &paint);

    paint.set_shader(checkerboard.clone());
    rect(canvas, &paint);

    // Blender that evaluates the "paint color" shader, with and without a shader on the paint
    paint.set_shader(None::<Shader>);
    paint.set_color_filter(None::<ColorFilter>);
    let blender_effect = RuntimeEffect::make_for_blender(
        "uniform shader s;\
         half4 main(half4 src, half4 dst) { return s.eval(float2(0)); }",
        None,
    )
    .expect("the blender compiles");
    paint.set_blender(blender_effect.make_blender(Data::new_empty(), &children));
    rect(canvas, &paint);

    paint.set_shader(checkerboard);
    rect(canvas, &paint);

    DrawResult::Ok
});
