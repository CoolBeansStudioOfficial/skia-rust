// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/workingspace.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::colors;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::color_space::{ColorSpace, named_transfer_fn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeEffect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::shaders::working_color_space_shader::WorkingColorSpaceShader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};

/// The bytes of an `SkColor4f`, as `SkData::MakeWithCopy(&color, sizeof(SkColor4f))` copies them.
// Port of: gm/workingspace.cpp#L22-L92 (chrome/m156), the `SkData::MakeWithCopy(&color, ...)` calls
fn color_bytes(color: Color4f) -> Data {
    let bytes: Vec<u8> = color
        .as_array()
        .iter()
        .flat_map(|c| c.to_ne_bytes())
        .collect();
    Data::new_copy(&bytes)
}

// Port of: gm/workingspace.cpp#L22-L33 (chrome/m156), color_shader
fn color_shader(color: Color4f) -> Option<Shader> {
    // Why not use SkShaders::Color? We want a shader that inhibits any paint optimization by any
    // backend. The CPU backend will ask the shader portion of the pipeline if it's constant.
    // If so, that portion of the pipeline is executed to get the color, and the color filter is
    // directly applied to the result. The only way to have the color filter run as part of the
    // full CPU pipeline is to have a shader that returns false for isConstant:
    let mut bmp = Bitmap::new();
    bmp.alloc_pixels_info(
        &ImageInfo::new(
            (1, 1),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None::<ColorSpace>,
        ),
        None,
    );
    bmp.erase_color_4f(color);
    bmp.to_shader(None, SamplingOptions::from(FilterMode::Nearest), None)
}

// Port of: gm/workingspace.cpp#L35-L42 (chrome/m156), paint_color_shader
fn paint_color_shader() -> Option<Shader> {
    // This will return the paint color (unless it's a child of a runtime effect)
    let mut bmp = Bitmap::new();
    bmp.alloc_pixels_info(&ImageInfo::new_a8((1, 1)), None);
    bmp.erase_color_4f(colors::WHITE);
    bmp.to_shader(None, SamplingOptions::from(FilterMode::Nearest), None)
}

// Port of: gm/workingspace.cpp#L44-L49 (chrome/m156), raw_shader
fn raw_shader(color: Color4f) -> Option<Shader> {
    RuntimeEffect::make_for_shader("uniform half4 c;half4 main(float2 xy) { return c; }", None)
        .expect("the raw shader compiles")
        .make_shader(color_bytes(color), &[], None)
}

// Port of: gm/workingspace.cpp#L51-L57 (chrome/m156), managed_shader
fn managed_shader(color: Color4f) -> Option<Shader> {
    RuntimeEffect::make_for_shader(
        "layout(color) uniform half4 c;half4 main(float2 xy) {return half4(c.rgb*c.a, c.a);}",
        None,
    )
    .expect("the managed shader compiles")
    .make_shader(color_bytes(color), &[], None)
}

// Port of: gm/workingspace.cpp#L59-L64 (chrome/m156), gradient_shader
fn gradient_shader() -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(40.0, 40.0)];
    let gradient_colors = [colors::RED, colors::GREEN];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&gradient_colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/workingspace.cpp#L66-L70 (chrome/m156), raw_cf
fn raw_cf(color: Color4f) -> Option<ColorFilter> {
    RuntimeEffect::make_for_color_filter(
        "uniform half4 c;half4 main(half4 color) { return c; }",
        None,
    )
    .expect("the raw color filter compiles")
    .make_color_filter(color_bytes(color), &[])
}

// Port of: gm/workingspace.cpp#L72-L76 (chrome/m156), managed_cf
fn managed_cf(color: Color4f) -> Option<ColorFilter> {
    RuntimeEffect::make_for_color_filter(
        "layout(color) uniform half4 c;half4 main(half4 color) { return c; }",
        None,
    )
    .expect("the managed color filter compiles")
    .make_color_filter(color_bytes(color), &[])
}

// Port of: gm/workingspace.cpp#L78-L84 (chrome/m156), indirect_cf
fn indirect_cf(color: Color4f) -> Option<ColorFilter> {
    let children = [ChildPtr::Shader(
        color_shader(color).expect("a color shader"),
    )];
    RuntimeEffect::make_for_color_filter(
        "uniform shader s;half4 main(half4 color) { return s.eval(float2(0)); }",
        None,
    )
    .expect("the indirect color filter compiles")
    .make_color_filter(Data::new_empty(), &children)
}

// Port of: gm/workingspace.cpp#L86-L88 (chrome/m156), mode_cf
fn mode_cf(color: Color4f) -> Option<ColorFilter> {
    color_filters::blend(color, None, BlendMode::Src)
}

// Port of: gm/workingspace.cpp#L90-L92 (chrome/m156), spin(ColorFilter)
fn spin_cf(cf: Option<ColorFilter>) -> Option<ColorFilter> {
    cf?.with_working_color_space(ColorSpace::new_srgb().with_color_spin())
}

// Port of: gm/workingspace.cpp#L94-L96 (chrome/m156), spin(Shader)
fn spin_shader(shader: Option<Shader>) -> Option<Shader> {
    Some(
        shader?
            .with_working_color_space(ColorSpace::new_srgb().with_color_spin(), None::<ColorSpace>),
    )
}

// Port of: gm/workingspace.cpp#L98-L100 (chrome/m156), linear(Shader)
fn linear(shader: Option<Shader>) -> Option<Shader> {
    Some(shader?.with_working_color_space(ColorSpace::new_srgb_linear(), None::<ColorSpace>))
}

// This GM checks that changing the working color space of a color filter does the right thing.
// We create a variety of color filters that are sensitive to the working space, and test them
// with both fixed-input color (so that paint optimization can apply the filter on the CPU),
// and with a shader input (so they're forced to evaluate in the drawing pipeline).
//
// In all cases, the tests are designed to draw green if implemented correctly. Any other color
// (red or blue, most likely) is an error.
//
// The bottom row is the exception - it draws red-to-green gradients. The first two should be
// "ugly" (via brown). The last one should be "nice" (via yellow).
// Port of: gm/workingspace.cpp#L102-L186 (chrome/m156), DEF_SIMPLE_GM_CAN_FAIL(workingspace)
//
// skia-rust: the harness always draws into a raster surface, so the GM's `getSurface()` skip (for
// the recording and DDL backends) is not reachable.
crate::def_simple_gm_can_fail!(workingspace, canvas, error_msg, 200, 350, {
    let _ = &error_msg;

    canvas.translate((5.0, 5.0));
    canvas.save();

    // Draws one 40x40 cell, then moves right by 50.
    let cell = |shader: Option<Shader>, color_filter: Option<ColorFilter>, paint_color: Color4f| {
        let mut paint = Paint::default();
        paint.set_color4f(paint_color, None);
        paint.set_shader(shader);
        paint.set_color_filter(color_filter);
        canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 40.0, 40.0), &paint);
        canvas.translate((50.0, 0.0));
    };

    // Moves down a row of cells.
    let next_row = || {
        canvas.restore();
        canvas.translate((0.0, 50.0));
        canvas.save();
    };

    let black_shader = color_shader(colors::BLACK);

    cell(None, raw_cf(colors::GREEN), colors::BLACK);
    cell(None, managed_cf(colors::GREEN), colors::BLACK);
    cell(None, indirect_cf(colors::GREEN), colors::BLACK);
    cell(None, mode_cf(colors::GREEN), colors::BLACK);

    next_row();

    cell(black_shader.clone(), raw_cf(colors::GREEN), colors::BLACK);
    cell(
        black_shader.clone(),
        managed_cf(colors::GREEN),
        colors::BLACK,
    );
    cell(
        black_shader.clone(),
        indirect_cf(colors::GREEN),
        colors::BLACK,
    );
    cell(black_shader.clone(), mode_cf(colors::GREEN), colors::BLACK);

    next_row();

    // Un-managed red turns into green
    cell(None, spin_cf(raw_cf(colors::RED)), colors::BLACK);
    cell(None, spin_cf(managed_cf(colors::GREEN)), colors::BLACK);
    cell(None, spin_cf(indirect_cf(colors::GREEN)), colors::BLACK);
    cell(None, spin_cf(mode_cf(colors::GREEN)), colors::BLACK);

    next_row();

    // Un-managed red turns into green
    cell(
        black_shader.clone(),
        spin_cf(raw_cf(colors::RED)),
        colors::BLACK,
    );
    cell(
        black_shader.clone(),
        spin_cf(managed_cf(colors::GREEN)),
        colors::BLACK,
    );
    cell(
        black_shader.clone(),
        spin_cf(indirect_cf(colors::GREEN)),
        colors::BLACK,
    );
    cell(
        black_shader.clone(),
        spin_cf(mode_cf(colors::GREEN)),
        colors::BLACK,
    );

    next_row();

    cell(raw_shader(colors::GREEN), None, colors::BLACK);
    cell(managed_shader(colors::GREEN), None, colors::BLACK);
    cell(color_shader(colors::GREEN), None, colors::BLACK);
    cell(paint_color_shader(), None, colors::GREEN);

    next_row();

    // Un-managed red turns into green
    cell(spin_shader(raw_shader(colors::RED)), None, colors::BLACK);
    cell(
        spin_shader(managed_shader(colors::GREEN)),
        None,
        colors::BLACK,
    );
    cell(
        spin_shader(color_shader(colors::GREEN)),
        None,
        colors::BLACK,
    );
    cell(spin_shader(paint_color_shader()), None, colors::GREEN);

    next_row();

    // Red to green, via ugly brown
    cell(gradient_shader(), None, colors::BLACK);
    // Same (spin doesn't change anything)
    cell(spin_shader(gradient_shader()), None, colors::BLACK);
    // Red to green, via bright yellow
    cell(linear(gradient_shader()), None, colors::BLACK);

    DrawResult::Ok
});

// When color conversion and alpha type is handled correctly from all input types (e.g. image,
// color, and runtime effect), this should produce a 2x3 grid of squares in the same green color.
//
// * For CPU/Ganesh, the bottom row (all workInUnpremul cases) render incorrectly because all inputs
//   still produce premul values and the wrapped shader is assumed to produce a premul value.
// * For Graphite, solid color and image inputs correctly convert to unpremul alpha but other shader
//   types produce premul values w/o any extra conversion to unpremul. Only the bottom-middle cell
//   renders incorrectly.
// Port of: gm/workingspace.cpp#L196-L266 (chrome/m156), DEF_SIMPLE_GM(workingspace_input_output)
crate::def_simple_gm!(workingspace_input_output, canvas, 256, 256, {
    // unpremul, sRGB input color to the runtime shader that is then wrapped in a workingspace
    let input_color = Color4f::new(0.2, 0.4, 0.7, 0.5);

    // These should produce the same value, barring colortype encoding precision
    let child_color = shaders::color_in_space(input_color, None::<ColorSpace>);
    let child_uniform = Some(managed_shader(input_color).expect("a managed shader"));
    let child_image = Some(color_shader(input_color).expect("a color shader"));

    // The input working space will be the linear space with the current surface's gamut
    let input_cs = canvas
        .image_info()
        .color_space()
        .map_or_else(ColorSpace::new_srgb_linear, |cs| cs.with_linear_gamma());

    // The output space will be a colorspin of the input gamut with a 2.2 gamma
    let output_cs = input_cs.with_color_spin();
    let output_gamut = output_cs.to_xyzd50();
    let output_cs = ColorSpace::new_rgb(&named_transfer_fn::DOT22, &output_gamut)
        .expect("the 2.2 gamma transfer function is valid");

    let rect = Rect::from_ltrb(0.0, 0.0, 32.0, 32.0);
    let padding = 4.0f32;

    canvas.translate((padding, padding));
    for work_in_unpremul in [false, true] {
        let manual_unpremul = if work_in_unpremul { "false" } else { "true" };
        let sksl = format!(
            "uniform shader child;\
             half4 main(float2 xy) {{\
                 half4 inRGBA = child.eval(xy);\
                 if ({manual_unpremul}) {{\
                     inRGBA.rgb /= inRGBA.a + 0.00001;\
                 }}\
                 half4 scaled = half4(saturate(inRGBA.rgb * 1.5), inRGBA.a);\
                 half4 outRGBA = half4(pow(scaled.bgr, half3(2.2)), scaled.a);\
                 if ({manual_unpremul}) {{\
                     outRGBA.rgb *= outRGBA.a;\
                 }}\
                 return outRGBA;\
             }}"
        );

        let effect =
            RuntimeEffect::make_for_shader(&sksl, None).expect("the runtime effect compiles");

        canvas.save();
        for child in [
            child_color.clone(),
            child_uniform.clone(),
            child_image.clone(),
        ] {
            let child = child.expect("a child shader");
            let mut builder =
                skia_rust_core::runtime_effect::RuntimeShaderBuilder::new(effect.clone());
            builder.child("child").assign(child);
            let rte_shader = builder.make_shader(None).expect("a runtime shader");

            let mut p = Paint::default();
            p.set_anti_alias(true);
            // The option that takes a workInUnpremul parameter isn't public yet
            p.set_shader(WorkingColorSpaceShader::make(
                rte_shader,
                Some(input_cs.clone()),
                Some(output_cs.clone()),
                work_in_unpremul,
            ));
            canvas.draw_rect(rect, &p);
            canvas.translate((rect.width() + padding, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, rect.height() + padding));
    }
});
