// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/runtimeshader.cpp (chrome/m156)

//! Runtime shader GMs.
//!
//! Ported here: `ThresholdRT`, `UnsharpRT`, `ColorCubeRT` and `ColorCubeColorFilterRT`, which load
//! their images through the codec crate. `ClipSuperRRect` is not in the manifest.

// SkIntToScalar of small values; the ported lambdas keep the C++ declaration order.
#![allow(clippy::cast_precision_loss, clippy::items_after_statements)]

use crate::prelude::*;
use crate::tool_utils::make_surface;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::{Color4f, colors};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::runtime_effect_priv;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::blur_filter;
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

// Port of: gm/runtimeshader.cpp#L92-L95 (chrome/m156), make_shader
fn make_shader_scaled(img: &Image, size: ISize) -> Shader {
    let scale = Matrix::scale((
        size.width as f32 / img.width() as f32,
        size.height as f32 / img.height() as f32,
    ));
    img.to_shader(None, SamplingOptions::default(), &scale)
        .expect("an image shader")
}

// Port of: gm/runtimeshader.cpp#L97-L127 (chrome/m156), make_threshold
fn make_threshold(size: ISize) -> Shader {
    let info = ImageInfo::new(
        (size.width, size.height),
        ColorType::Alpha8,
        AlphaType::Premul,
        None::<skia_rust_core::color_space::ColorSpace>,
    );
    let mut surf = surfaces::raster(&info, None, None).expect("a surface");
    {
        let canvas = surf.canvas();

        let rad = 50.0_f32;
        let colors = [colors::BLACK, Color4f::new(0.0, 0.0, 0.0, 0.0)];
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_shader(gradient_shaders::radial_gradient(
            (skia_rust_core::point::Point::new(0.0, 0.0), rad),
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));

        let mut layer_paint = Paint::default();
        let sigma = 16.0_f32;
        layer_paint.set_image_filter(blur_filter::blur(sigma, sigma, TileMode::Decal, None, None));
        canvas.save_layer(&SaveLayerRec::default().paint(&layer_paint));

        let mut random = Random::default();
        for _ in 0..25 {
            let x = random.next_f() * size.width as f32;
            let y = random.next_f() * size.height as f32;
            canvas.save();
            canvas.translate((x, y));
            canvas.draw_circle((0.0, 0.0), rad, &paint);
            canvas.restore();
        }

        canvas.restore(); // apply the blur
    }

    surf.image_snapshot()
        .expect("a snapshot")
        .to_shader(None, SamplingOptions::default(), None)
        .expect("an image shader")
}

const THRESHOLD_SKSL: &str = r"
        uniform shader before_map;
        uniform shader after_map;
        uniform shader threshold_map;

        uniform float cutoff;
        uniform float slope;

        float smooth_cutoff(float x) {
            x = x * slope + (0.5 - slope * cutoff);
            return clamp(x, 0, 1);
        }

        half4 main(float2 xy) {
            half4 before = before_map.eval(xy);
            half4 after = after_map.eval(xy);

            float m = smooth_cutoff(threshold_map.eval(xy).a);
            return mix(before, after, m);
        }
    ";

// Port of: gm/runtimeshader.cpp#L129-L190 (chrome/m156), class ThresholdRT
struct ThresholdRt {
    base: RuntimeShaderGm,
    before: Option<Shader>,
    after: Option<Shader>,
    threshold: Option<Shader>,
}

impl ThresholdRt {
    fn new() -> Self {
        ThresholdRt {
            base: RuntimeShaderGm::new(
                "threshold_rt",
                ISize::new(256, 256),
                THRESHOLD_SKSL,
                K_ANIMATE_RT_FLAG | K_BENCH_RT_FLAG,
            ),
            before: None,
            after: None,
            threshold: None,
        }
    }
}

impl GM for ThresholdRt {
    fn name(&self) -> String {
        self.base.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.base.size
    }

    // Port of: gm/runtimeshader.cpp#L157-L166 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let size = ISize::new(256, 256);
        self.threshold = Some(make_threshold(size));
        self.before = Some(make_shader_scaled(
            &crate::tool_utils::get_resource_as_image("images/mandrill_256.png")
                .expect("images/mandrill_256.png"),
            size,
        ));
        self.after = Some(make_shader_scaled(
            &crate::tool_utils::get_resource_as_image("images/dog.jpg").expect("images/dog.jpg"),
            size,
        ));
        self.base.on_once_before_draw();
    }

    // Port of: gm/runtimeshader.cpp#L168-L188 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut builder = self.base.builder();

        builder
            .uniform("cutoff")
            .set_f32(&[(self.base.secs).sin() * 0.55 + 0.5]);
        builder.uniform("slope").set_f32(&[10.0]);

        builder
            .child("before_map")
            .assign(self.before.clone().expect("before"));
        builder
            .child("after_map")
            .assign(self.after.clone().expect("after"));
        builder
            .child("threshold_map")
            .assign(self.threshold.clone().expect("threshold"));

        let mut paint = Paint::default();
        paint.set_shader(builder.make_shader(None));
        canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 256.0, 256.0), &paint);

        let mut draw = |x: f32, y: f32, shader: Option<Shader>| {
            paint.set_shader(shader);
            canvas.save();
            canvas.translate((x, y));
            canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 256.0, 256.0), &paint);
            canvas.restore();
        };
        draw(256.0, 0.0, self.threshold.clone());
        draw(0.0, 256.0, self.before.clone());
        draw(256.0, 256.0, self.after.clone());
    }
}

// Port of: gm/runtimeshader.cpp#L190 (chrome/m156), DEF_GM(return new ThresholdRT;)
crate::def_gm!(ThresholdRT, ThresholdRt::new());

const UNSHARP_SKSL: &str = r"
        uniform shader child;
        half4 main(float2 xy) {
            half4 c = child.eval(xy) * 5;
            c -= child.eval(xy + float2( 1,  0));
            c -= child.eval(xy + float2(-1,  0));
            c -= child.eval(xy + float2( 0,  1));
            c -= child.eval(xy + float2( 0, -1));
            return c;
        }
    ";

// Port of: gm/runtimeshader.cpp#L231-L267 (chrome/m156), class UnsharpRT
struct UnsharpRt {
    base: RuntimeShaderGm,
    mandrill: Option<Image>,
}

impl UnsharpRt {
    fn new() -> Self {
        UnsharpRt {
            base: RuntimeShaderGm::new("unsharp_rt", ISize::new(512, 256), UNSHARP_SKSL, 0),
            mandrill: None,
        }
    }
}

impl GM for UnsharpRt {
    fn name(&self) -> String {
        self.base.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.base.size
    }

    // Port of: gm/runtimeshader.cpp#L240-L244 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.mandrill = crate::tool_utils::get_resource_as_image("images/mandrill_256.png");
        self.base.on_once_before_draw();
    }

    // Port of: gm/runtimeshader.cpp#L246-L262 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mandrill = self.mandrill.clone().expect("images/mandrill_256.png");
        // First we draw the unmodified image
        canvas.draw_image(&mandrill, (0.0, 0.0), None);

        // Now draw the image with our unsharp mask applied
        let mut builder = self.base.builder();
        let sampling = SamplingOptions::from(FilterMode::Nearest);
        builder.child("child").assign(
            mandrill
                .to_shader(None, sampling, None)
                .expect("an image shader"),
        );

        let mut paint = Paint::default();
        paint.set_shader(builder.make_shader(None));
        canvas.translate((256.0, 0.0));
        canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 256.0, 256.0), &paint);
    }
}

// Port of: gm/runtimeshader.cpp#L267 (chrome/m156), DEF_GM(return new UnsharpRT;)
crate::def_gm!(UnsharpRT, UnsharpRt::new());

const COLOR_CUBE_SKSL: &str = r"
        uniform shader child;
        uniform shader color_cube;
        uniform float rg_scale;
        uniform float rg_bias;
        uniform float b_scale;
        uniform float inv_size;
        half4 main(float2 xy) {
            float4 c = unpremul(child.eval(xy));
            // Map to cube coords:
            float3 cubeCoords = float3(c.rg * rg_scale + rg_bias, c.b * b_scale);
            // Compute slice coordinate
            float2 coords1 = float2((floor(cubeCoords.b) + cubeCoords.r) * inv_size, cubeCoords.g);
            float2 coords2 = float2(( ceil(cubeCoords.b) + cubeCoords.r) * inv_size, cubeCoords.g);
            // Two bilinear fetches, plus a manual lerp for the third axis:
            half4 color = mix(color_cube.eval(coords1), color_cube.eval(coords2),
                              fract(cubeCoords.b));
            // Premul again
            color.rgb *= color.a;
            return color;
        }
    ";

// The LUT images shared by the two colour-cube GMs.
struct CubeImages {
    mandrill: Image,
    mandrill_sepia: Image,
    identity_cube: Image,
    sepia_cube: Image,
}

impl CubeImages {
    // Port of: gm/runtimeshader.cpp#L281-L286 (chrome/m156), onOnceBeforeDraw (image loads)
    fn load() -> Self {
        let load = |path: &str| {
            crate::tool_utils::get_resource_as_image(path).unwrap_or_else(|| panic!("{path}"))
        };
        CubeImages {
            mandrill: load("images/mandrill_256.png"),
            mandrill_sepia: load("images/mandrill_sepia.png"),
            identity_cube: load("images/lut_identity.png"),
            sepia_cube: load("images/lut_sepia.png"),
        }
    }
}

// Port of: gm/runtimeshader.cpp#L269-L349 (chrome/m156), class ColorCubeRT
struct ColorCubeRt {
    base: RuntimeShaderGm,
    images: Option<CubeImages>,
}

impl ColorCubeRt {
    fn new() -> Self {
        ColorCubeRt {
            base: RuntimeShaderGm::new("color_cube_rt", ISize::new(512, 512), COLOR_CUBE_SKSL, 0),
            images: None,
        }
    }
}

impl GM for ColorCubeRt {
    fn name(&self) -> String {
        self.base.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.base.size
    }

    fn on_once_before_draw(&mut self) {
        self.images = Some(CubeImages::load());
        self.base.on_once_before_draw();
    }

    // Port of: gm/runtimeshader.cpp#L296-L340 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let images = self.images.as_ref().expect("the images are loaded");
        let mut builder = self.base.builder();

        // First we draw the unmodified image, and a copy that was sepia-toned in Photoshop:
        canvas.draw_image(&images.mandrill, (0.0, 0.0), None);
        canvas.draw_image(&images.mandrill_sepia, (0.0, 256.0), None);

        // LUT dimensions should be (kSize^2, kSize)
        let k_size = 16.0_f32;
        let sampling = SamplingOptions::from(FilterMode::Linear);
        builder
            .uniform("rg_scale")
            .set_f32(&[(k_size - 1.0) / k_size]);
        builder.uniform("rg_bias").set_f32(&[0.5 / k_size]);
        builder.uniform("b_scale").set_f32(&[k_size - 1.0]);
        builder.uniform("inv_size").set_f32(&[1.0 / k_size]);
        builder.child("child").assign(
            images
                .mandrill
                .to_shader(None, sampling, None)
                .expect("an image shader"),
        );

        let mut paint = Paint::default();
        // TODO: Should we add SkImage::makeNormalizedShader() to handle this automatically?
        let normalize = Matrix::scale((1.0 / (k_size * k_size), 1.0 / k_size));

        // Now draw the image with an identity color cube - it should look like the original
        builder.child("color_cube").assign(
            images
                .identity_cube
                .to_shader(None, sampling, &normalize)
                .expect("an image shader"),
        );
        paint.set_shader(builder.make_shader(None));
        canvas.translate((256.0, 0.0));
        canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 256.0, 256.0), &paint);

        // ... and with a sepia-tone color cube. This should match the sepia-toned image.
        builder.child("color_cube").assign(
            images
                .sepia_cube
                .to_shader(None, sampling, &normalize)
                .expect("an image shader"),
        );
        paint.set_shader(builder.make_shader(None));
        canvas.translate((0.0, 256.0));
        canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 256.0, 256.0), &paint);
    }
}

// Port of: gm/runtimeshader.cpp#L349 (chrome/m156), DEF_GM(return new ColorCubeRT;)
crate::def_gm!(ColorCubeRT, ColorCubeRt::new());

const COLOR_CUBE_CF_SKSL: &str = r"
        uniform shader color_cube;
        uniform float rg_scale;
        uniform float rg_bias;
        uniform float b_scale;
        uniform float inv_size;
        half4 main(half4 inColor) {
            float4 c = unpremul(inColor);
            float3 cubeCoords = float3(c.rg * rg_scale + rg_bias, c.b * b_scale);
            float2 coords1 = float2((floor(cubeCoords.b) + cubeCoords.r) * inv_size, cubeCoords.g);
            float2 coords2 = float2(( ceil(cubeCoords.b) + cubeCoords.r) * inv_size, cubeCoords.g);
            half4 color = mix(color_cube.eval(coords1), color_cube.eval(coords2),
                              fract(cubeCoords.b));
            color.rgb *= color.a;
            return color;
        }
    ";

// Port of: gm/runtimeshader.cpp#L353-L430 (chrome/m156), class ColorCubeColorFilterRT
struct ColorCubeColorFilterRt {
    base: RuntimeShaderGm,
    images: Option<CubeImages>,
}

impl ColorCubeColorFilterRt {
    fn new() -> Self {
        ColorCubeColorFilterRt {
            base: RuntimeShaderGm::new(
                "color_cube_cf_rt",
                ISize::new(512, 512),
                COLOR_CUBE_CF_SKSL,
                0,
            ),
            images: None,
        }
    }
}

impl GM for ColorCubeColorFilterRt {
    fn name(&self) -> String {
        self.base.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.base.size
    }

    // Port of: gm/runtimeshader.cpp#L362-L372 (chrome/m156), onOnceBeforeDraw; the effect is a
    // colour filter (SkRuntimeEffect::MakeForColorFilter), not a shader.
    fn on_once_before_draw(&mut self) {
        self.images = Some(CubeImages::load());
        match RuntimeEffect::make_for_color_filter(COLOR_CUBE_CF_SKSL, None) {
            Ok(effect) => self.base.effect = Some(effect),
            Err(error) => eprintln!("RuntimeShader error: {error}"),
        }
    }

    // Port of: gm/runtimeshader.cpp#L374-L409 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let images = self.images.as_ref().expect("the images are loaded");
        let mut builder = self.base.builder();

        canvas.draw_image(&images.mandrill, (0.0, 0.0), None);
        canvas.draw_image(&images.mandrill_sepia, (0.0, 256.0), None);

        let k_size = 16.0_f32;
        let sampling = SamplingOptions::from(FilterMode::Linear);
        builder
            .uniform("rg_scale")
            .set_f32(&[(k_size - 1.0) / k_size]);
        builder.uniform("rg_bias").set_f32(&[0.5 / k_size]);
        builder.uniform("b_scale").set_f32(&[k_size - 1.0]);
        builder.uniform("inv_size").set_f32(&[1.0 / k_size]);

        let mut paint = Paint::default();
        let normalize = Matrix::scale((1.0 / (k_size * k_size), 1.0 / k_size));

        builder.child("color_cube").assign(
            images
                .identity_cube
                .to_shader(None, sampling, &normalize)
                .expect("an image shader"),
        );
        paint.set_color_filter(builder.make_color_filter());
        canvas.draw_image_with_sampling_options(
            &images.mandrill,
            (256.0, 0.0),
            sampling,
            Some(&paint),
        );

        builder.child("color_cube").assign(
            images
                .sepia_cube
                .to_shader(None, sampling, &normalize)
                .expect("an image shader"),
        );
        paint.set_color_filter(builder.make_color_filter());
        canvas.draw_image_with_sampling_options(
            &images.mandrill,
            (256.0, 256.0),
            sampling,
            Some(&paint),
        );
    }
}

// Port of: gm/runtimeshader.cpp#L430 (chrome/m156), DEF_GM(return new ColorCubeColorFilterRT;)
crate::def_gm!(ColorCubeColorFilterRT, ColorCubeColorFilterRt::new());

// Port of: gm/runtimeshader.cpp#L862-L907 (chrome/m156), DEF_SIMPLE_GM(local_matrix_shader_rt)
crate::def_simple_gm!(local_matrix_shader_rt, canvas, 256, 256, {
    let passthrough = r"
        uniform shader s;
        half4 main(float2 p) { return s.eval(p); }
    ";
    let Ok(rte) = RuntimeEffect::make_for_shader(passthrough, None) else {
        eprintln!("RuntimeShader error");
        return;
    };
    let image = crate::tool_utils::get_resource_as_image("images/mandrill_128.png")
        .expect("images/mandrill_128.png");
    let sampling = SamplingOptions::from(FilterMode::Nearest);
    let img_shader = image
        .to_shader(None, sampling, None)
        .expect("an image shader");
    let r = Rect::from_ltrb(0.0, 0.0, image.width() as f32, image.height() as f32);
    let lm = Matrix::rotate_deg_pivot(
        90.0,
        (image.width() as f32 / 2.0, image.height() as f32 / 2.0),
    );
    let mut paint = Paint::default();
    // image
    paint.set_shader(Some(img_shader.clone()));
    canvas.draw_rect(r, &paint);
    // passthrough(image)
    canvas.save();
    canvas.translate((image.width() as f32, 0.0));
    paint.set_shader(rte.make_shader(
        Data::new_empty(),
        &[ChildPtr::from(img_shader.clone())],
        None,
    ));
    canvas.draw_rect(r, &paint);
    canvas.restore();
    // localmatrix(image)
    canvas.save();
    canvas.translate((0.0, image.height() as f32));
    paint.set_shader(Some(img_shader.with_local_matrix(&lm)));
    canvas.draw_rect(r, &paint);
    canvas.restore();
    // localmatrix(passthrough(image)) This was the bug.
    canvas.save();
    canvas.translate((image.width() as f32, image.height() as f32));
    let passed = rte
        .make_shader(
            Data::new_empty(),
            &[ChildPtr::from(img_shader.clone())],
            None,
        )
        .expect("a shader");
    paint.set_shader(Some(passed.with_local_matrix(&lm)));
    canvas.draw_rect(r, &paint);
    canvas.restore();
});

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

// Port of: gm/runtimeshader.cpp#L440-L590 (chrome/m156), ClipSuperRRect
struct ClipSuperRRectGm {
    base: RuntimeShaderGm,
    power: f32,
}

impl ClipSuperRRectGm {
    // Port of: gm/runtimeshader.cpp#L444-L460 (chrome/m156), the constructor
    fn new(name: &'static str, power: f32) -> Self {
        ClipSuperRRectGm {
            base: RuntimeShaderGm::new(
                name,
                ISize::new(500, 500),
                r"
        uniform float power_minus1;
        uniform float2 stretch_factor;
        uniform float2x2 derivatives;
        half4 main(float2 xy) {
            xy = max(abs(xy) + stretch_factor, 0);
            float2 exp_minus1 = pow(xy, power_minus1.xx);  // If power == 3.5: xy * xy * sqrt(xy)
            float f = dot(exp_minus1, xy) - 1;  // f = x^n + y^n - 1
            float2 grad = exp_minus1 * derivatives;
            float fwidth = abs(grad.x) + abs(grad.y) + 1e-12;  // 1e-12 to avoid a divide by zero.
            return half4(saturate(.5 - f/fwidth)); // Approx coverage by riding the gradient to f=0.
        }
    ",
                0,
            ),
            power,
        }
    }

    // Port of: gm/runtimeshader.cpp#L462-L527 (chrome/m156), drawSuperRRect
    #[allow(clippy::float_cmp)] // mirrors `fPower == 2` in the C++
    #[allow(clippy::many_single_char_names)] // the C++ names: a, b, c and d are the matrix entries
    fn draw_super_rrect(
        &self,
        canvas: &Canvas,
        super_rrect: Rect,
        rad_x: f32,
        rad_y: f32,
        color: Color,
    ) {
        let mut paint = Paint::default();
        paint.set_color(color);
        if self.power == 2.0 {
            // Draw a normal round rect for the sake of testing.
            let rrect = skia_rust_core::rrect::RRect::new_rect_xy(super_rrect, rad_x, rad_y);
            paint.set_anti_alias(true);
            canvas.draw_rrect(rrect, &paint);
            return;
        }
        let mut builder = self.base.builder();
        builder.uniform("power_minus1").set_f32(&[self.power - 1.0]);
        // Size the corners such that the "apex" of our "super" rounded corner is in the same
        // location that the apex of a circular rounded corner would be with the given radii. We
        // define the apex as the point on the rounded corner that is 45 degrees between the
        // horizontal and vertical edges.
        let scale = (1.0 - skia_rust_core::scalar::SCALAR_ROOT_2_OVER_2)
            / (1.0 - (-1.0 / self.power).exp2());
        let mut corner_width = rad_x * scale;
        let mut corner_height = rad_y * scale;
        corner_width = corner_width.min(super_rrect.width() * 0.5);
        corner_height = corner_height.min(super_rrect.height() * 0.5);
        // The stretch factor controls how long the flat edge should be between rounded corners.
        builder.uniform("stretch_factor").set_f32(&[
            1.0 - super_rrect.width() * 0.5 / corner_width,
            1.0 - super_rrect.height() * 0.5 / corner_height,
        ]);
        // Calculate a 2x2 "derivatives" matrix that the shader will use to find the gradient.
        //
        //     f = s^n + t^n - 1   [s,t are "super" rounded corner coords in normalized 0..1 space]
        //
        //     gradient = [df/dx  df/dy] = [ns^(n-1)  nt^(n-1)] * |ds/dx  ds/dy|
        //                                                        |dt/dx  dt/dy|
        //
        //              = [s^(n-1)  t^(n-1)] * |n  0| * |ds/dx  ds/dy|
        //                                     |0  n|   |dt/dx  dt/dy|
        //
        //              = [s^(n-1)  t^(n-1)] * |2n/cornerWidth   0| * mat2x2(canvasMatrix)^-1
        //                                     |0  2n/cornerHeight|
        //
        //              = [s^(n-1)  t^(n-1)] * "derivatives"
        //
        let m = canvas.total_matrix();
        let (a, b, c, d) = (m.scale_x(), m.skew_x(), m.skew_y(), m.scale_y());
        let determinant = a * d - b * c;
        let dx = self.power / (corner_width * determinant);
        let dy = self.power / (corner_height * determinant);
        builder
            .uniform("derivatives")
            .set_f32(&[d * dx, -c * dy, -b * dx, a * dy]);
        // This matrix will be inverted by the effect system, giving a matrix that converts local
        // coordinates to (almost) coner coordinates. To get the rest of the way to the nearest
        // corner's space, the shader will have to take the absolute value, add the stretch_factor,
        // then clamp above zero.
        let mut corner_to_local = Matrix::new_identity();
        corner_to_local.set_scale_translate(
            (corner_width, corner_height),
            (super_rrect.center_x(), super_rrect.center_y()),
        );
        if let Some(clip) = builder.make_shader(Some(&corner_to_local)) {
            canvas.clip_shader(clip, None);
        }
        // Bloat the outer edges of the rect we will draw so it contains all the antialiased pixels.
        // Bloat by a full pixel instead of half in case Skia is in a mode that draws this rect with
        // unexpected AA of its own.
        let inverse_det = 1.0 / determinant.abs();
        let bloat_x = (d.abs() + c.abs()) * inverse_det;
        let bloat_y = (b.abs() + a.abs()) * inverse_det;
        let mut outset = super_rrect;
        outset.outset((bloat_x, bloat_y));
        canvas.draw_rect(outset, &paint);
    }
}

impl GM for ClipSuperRRectGm {
    fn name(&self) -> String {
        self.base.name.to_string()
    }

    fn size(&mut self) -> ISize {
        self.base.size
    }

    fn on_once_before_draw(&mut self) {
        self.base.on_once_before_draw();
    }

    // Port of: gm/runtimeshader.cpp#L500-L586 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut rand = Random::new(2);
        let info = canvas.image_info();
        canvas.save();
        canvas.translate((
            crate::tool_utils::int_to_scalar(info.width()) / 2.0,
            crate::tool_utils::int_to_scalar(info.height()) / 2.0,
        ));
        let entries: [(f32, Rect, f32, f32); 8] = [
            (21.0, Rect::new(-5.0, 25.0, 170.0, 125.0), 50.0, 30.0),
            (94.0, Rect::new(95.0, 75.0, 220.0, 175.0), 30.0, 30.0),
            (132.0, Rect::new(0.0, 75.0, 150.0, 175.0), 40.0, 30.0),
            (282.0, Rect::new(15.0, -20.0, 115.0, 80.0), 20.0, 20.0),
            (0.0, Rect::new(140.0, -50.0, 230.0, 60.0), 25.0, 25.0),
            (-35.0, Rect::new(160.0, -60.0, 220.0, 30.0), 18.0, 18.0),
            (65.0, Rect::new(220.0, -120.0, 280.0, -30.0), 18.0, 18.0),
            (265.0, Rect::new(150.0, -129.0, 230.0, 31.0), 24.0, 39.0),
        ];
        for (angle, rect, rad_x, rad_y) in entries {
            canvas.save();
            canvas.rotate(angle, None);
            self.draw_super_rrect(
                canvas,
                rect,
                rad_x,
                rad_y,
                Color::from(rand.next_u() | 0xff80_8080),
            );
            canvas.restore();
        }
        canvas.restore();
    }
}

// Port of: gm/runtimeshader.cpp#L588 (chrome/m156), DEF_GM(return new ClipSuperRRect("clip_super_rrect_pow2", 2);)
crate::def_gm!(
    ClipSuperRRect_pow2 = "ClipSuperRRect(\"clip_super_rrect_pow2\", 2)",
    ClipSuperRRectGm::new("clip_super_rrect_pow2", 2.0)
);
// Port of: gm/runtimeshader.cpp#L590 (chrome/m156), DEF_GM(return new ClipSuperRRect("clip_super_rrect_pow3.5", 3.5);)
crate::def_gm!(
    ClipSuperRRect_pow3_5 = "ClipSuperRRect(\"clip_super_rrect_pow3.5\", 3.5)",
    ClipSuperRRectGm::new("clip_super_rrect_pow3.5", 3.5)
);
