// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/arithmode.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: int/float conversions, local constants and long
// bodies are kept as they are there.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::too_many_lines
)]

use crate::prelude::*;
use crate::tool_utils::create_checkerboard_image;
use skia_rust_core::blender::Blender;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::data::Data;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeEffect};
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::string::str_append_scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::blenders;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::{arithmetic, blend_with_blender, image_sampled};
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const WW: i32 = 100;
const HH: i32 = 32;

/// `make_src(w, h)`: a linear gradient through transparent, green, cyan, red, magenta and white.
// Port of: gm/arithmode.cpp#L16-L27 (chrome/m156)
fn make_src(w: i32, h: i32) -> Option<Image> {
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 0.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 1.0, 1.0),
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(1.0, 0.0, 1.0, 1.0),
        Color4f::new(1.0, 1.0, 1.0, 1.0),
    ];
    let pts = (Point::new(0.0, 0.0), Point::new(w as f32, h as f32));
    draw_gradient_image(w, h, pts, &colors)
}

/// `make_dst(w, h)`: a gradient from the bottom left through blue, yellow, black, green and gray.
// Port of: gm/arithmode.cpp#L29-L43 (chrome/m156)
fn make_dst(w: i32, h: i32) -> Option<Image> {
    // matches SK_ColorGRAY
    let compat_gray = f32::from(0x88_u8) / 255.0;
    let colors = [
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(1.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(compat_gray, compat_gray, compat_gray, 1.0),
    ];
    let pts = (Point::new(0.0, h as f32), Point::new(w as f32, 0.0));
    draw_gradient_image(w, h, pts, &colors)
}

/// Draws the clamped linear gradient `pts` through `colors` over a `w` by `h` premultiplied
/// surface, and snapshots it.
fn draw_gradient_image(w: i32, h: i32, pts: (Point, Point), colors: &[Color4f]) -> Option<Image> {
    let info = ImageInfo::new_n32_premul((w, h), None);
    let mut surf = surfaces::raster(&info, None, None)?;
    let mut paint = Paint::default();
    paint.set_shader(gradient_shaders::linear_gradient(
        pts,
        &Gradient::new(
            Colors::new(colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    surf.canvas().draw_paint(&paint);
    surf.image_snapshot()
}

/// `show_k_text(canvas, x, y, k)`: the four coefficients, side by side.
// Port of: gm/arithmode.cpp#L45-L58 (chrome/m156)
fn show_k_text(canvas: &Canvas, x: f32, y: f32, k: &[f32; 4]) {
    let mut font = Font::from_size(default_portable_typeface(), 24.0);
    font.set_edging(Edging::AntiAlias);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let mut x = x;
    for value in k {
        let mut text = String::new();
        str_append_scalar(&mut text, *value);
        let (width, _) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        canvas.draw_str(&text, (x, y + font.size()), &font, &paint);
        x += width + 10.0;
    }
}

/// `ArithmodeGM`: the arithmetic image filter for a table of coefficients.
// Port of: gm/arithmode.cpp#L60-L121 (chrome/m156)
struct ArithmodeGm;

impl GM for ArithmodeGm {
    fn name(&self) -> String {
        "arithmode".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 572)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let (Some(src), Some(dst)) = (make_src(WW, HH), make_dst(WW, HH)) else {
            return;
        };
        let linear = SamplingOptions::new(FilterMode::Linear, MipmapMode::None);
        let src_filter: Option<ImageFilter> = image_sampled(Some(src.clone()), linear);
        let dst_filter: Option<ImageFilter> = image_sampled(Some(dst.clone()), linear);
        let one = 1.0_f32;
        let k_table: [[f32; 4]; 11] = [
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, one],
            [0.0, one, 0.0, 0.0],
            [0.0, 0.0, one, 0.0],
            [0.0, one, one, 0.0],
            [0.0, one, -one, 0.0],
            [0.0, one / 2.0, one / 2.0, 0.0],
            [0.0, one / 2.0, one / 2.0, one / 4.0],
            [0.0, one / 2.0, one / 2.0, -one / 4.0],
            [one / 4.0, one / 2.0, one / 2.0, 0.0],
            [-one / 4.0, one / 2.0, one / 2.0, 0.0],
        ];

        // Many of the Arithmetic filters have a 4th coefficient that's not zero, which means they
        // affect transparent black. 'rect' is used as a crop filter to make sure they don't
        // overwrite each other.
        let rect = Rect::from_wh(WW as f32, HH as f32);
        let gap = (WW + 20) as f32;
        for k in k_table {
            canvas.save();
            canvas.draw_image(&src, (0.0, 0.0), None);
            canvas.translate((gap, 0.0));
            canvas.draw_image(&dst, (0.0, 0.0), None);
            canvas.translate((gap, 0.0));
            let mut paint = Paint::default();
            paint.set_image_filter(arithmetic(
                k[0],
                k[1],
                k[2],
                k[3],
                true,
                dst_filter.clone(),
                src_filter.clone(),
                Some(rect),
            ));
            canvas.save_layer(&SaveLayerRec::default().paint(&paint));
            canvas.restore();
            canvas.translate((gap, 0.0));
            show_k_text(canvas, 0.0, 0.0, &k);
            canvas.restore();
            canvas.translate((0.0, (HH + 12) as f32));
        }

        // Draw two special cases to test enforcePMColor. In these cases, we draw the dst bitmap
        // twice, the first time it is halved and inverted, leading to invalid premultiplied
        // colors. If we enforcePMColor, these invalid values should be clamped, and will not
        // contribute to the second draw.
        for i in 0..2 {
            let enforce_pm_color = i == 0;
            canvas.save();
            canvas.translate((gap, 0.0));
            canvas.draw_image(&dst, (0.0, 0.0), None);
            canvas.translate((gap, 0.0));
            let bg = arithmetic(
                0.0,
                0.0,
                -one / 2.0,
                1.0,
                enforce_pm_color,
                dst_filter.clone(),
                None,
                None,
            );
            let mut p = Paint::default();
            p.set_image_filter(arithmetic(
                0.0,
                one / 2.0,
                -one,
                1.0,
                true,
                bg,
                dst_filter.clone(),
                Some(rect),
            ));
            canvas.save_layer(&SaveLayerRec::default().paint(&p));
            canvas.restore();
            canvas.translate((gap, 0.0));
            // Label
            let font = Font::from_size(default_portable_typeface(), 24.0);
            let label = if enforce_pm_color {
                "enforcePM"
            } else {
                "no enforcePM"
            };
            canvas.draw_str(label, (0.0, font.size()), &font, &Paint::default());
            canvas.restore();
            canvas.translate((0.0, (HH + 12) as f32));
        }
    }
}

/// `ArithmodeBlenderGM`: the arithmetic blender through a blend, an image filter, a shader and a
/// runtime effect, which should all look the same.
// Port of: gm/arithmode.cpp#L123-L282 (chrome/m156)
struct ArithmodeBlenderGm {
    k1: f32,
    k2: f32,
    k3: f32,
    k4: f32,
    src: Option<Image>,
    dst: Option<Image>,
    checker: Option<Image>,
    src_shader: Option<Shader>,
    dst_shader: Option<Shader>,
    runtime_effect: Option<RuntimeEffect>,
}

const BLENDER_W: i32 = 200;
const BLENDER_H: i32 = 200;

impl GM for ArithmodeBlenderGm {
    fn name(&self) -> String {
        "arithmode_blender".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new((BLENDER_W + 30) * 2, (BLENDER_H + 30) * 4)
    }

    // Port of: gm/arithmode.cpp#L140-L162 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        // Prepare a runtime effect for this blend.
        const SKSL: &str = "uniform shader srcImage;\
            uniform shader dstImage;\
            uniform blender arithBlend;\
            half4 main(float2 xy) {\
                return arithBlend.eval(srcImage.eval(xy), dstImage.eval(xy));\
            }";
        self.runtime_effect = Some(
            RuntimeEffect::make_for_shader(SKSL, None)
                .expect("the arithmode_blender shader compiles"),
        );

        // Start with interesting K-values, in case we're drawn without calling onAnimate().
        self.k1 = -0.25;
        self.k2 = 0.25;
        self.k3 = 0.25;
        self.k4 = 0.0;
        let src = make_src(BLENDER_W, BLENDER_H);
        let dst = make_dst(BLENDER_W, BLENDER_H);
        let clamp = Some((TileMode::Clamp, TileMode::Clamp));
        self.src_shader = src
            .as_ref()
            .and_then(|image| image.to_shader(clamp, SamplingOptions::default(), None));
        self.dst_shader = dst
            .as_ref()
            .and_then(|image| image.to_shader(clamp, SamplingOptions::default(), None));
        self.src = src;
        self.dst = dst;
        self.checker = Some(create_checkerboard_image(
            BLENDER_W,
            BLENDER_H,
            Color::new(0xFFBB_BBBB),
            Color::new(0xFFEE_EEEE),
            8,
        ));
    }

    // Port of: gm/arithmode.cpp#L205-L266 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let (Some(src), Some(dst), Some(checker), Some(src_shader), Some(dst_shader), Some(effect)) = (
            self.src.as_ref(),
            self.dst.as_ref(),
            self.checker.as_ref(),
            self.src_shader.as_ref(),
            self.dst_shader.as_ref(),
            self.runtime_effect.as_ref(),
        ) else {
            return;
        };
        let w = BLENDER_W as f32;
        let h = BLENDER_H as f32;
        let rect = Rect::from_wh(w, h);
        canvas.draw_image(src, (10.0, 10.0), None);
        canvas.draw_image(dst, (10.0, 10.0 + h + 10.0), None);

        let sampling = SamplingOptions::default();
        let blender: Blender = blenders::arithmetic(self.k1, self.k2, self.k3, self.k4, true)
            .expect("an arithmetic blender");
        canvas.translate((10.0 + w + 10.0, 10.0));

        // All three images drawn below should appear identical.
        // Draw via blend step
        let mut blender_paint = Paint::default();
        canvas.draw_image(checker, (0.0, 0.0), None);
        canvas.save_layer(&SaveLayerRec::default().bounds(&rect));
        canvas.draw_image(dst, (0.0, 0.0), None);
        blender_paint.set_blender(blender.clone());
        canvas.draw_image_with_sampling_options(src, (0.0, 0.0), sampling, Some(&blender_paint));
        canvas.restore();
        canvas.translate((0.0, 10.0 + h));

        // Draw via SkImageFilters::Blend (should appear the same as above)
        let mut image_filter_paint = Paint::default();
        canvas.draw_image(checker, (0.0, 0.0), None);
        image_filter_paint.set_image_filter(blend_with_blender(
            blender.clone(),
            None,
            image_sampled(Some(src.clone()), sampling),
            None,
        ));
        canvas.draw_image_with_sampling_options(
            dst,
            (0.0, 0.0),
            sampling,
            Some(&image_filter_paint),
        );
        canvas.translate((0.0, 10.0 + h));

        // Draw via SkShaders::Blend (should still appear the same as above)
        let mut shader_blend_paint = Paint::default();
        canvas.draw_image(checker, (0.0, 0.0), None);
        shader_blend_paint.set_shader(shaders::blend_blender(
            &blender,
            dst_shader.clone(),
            src_shader.clone(),
        ));
        canvas.draw_rect(rect, &shader_blend_paint);
        canvas.translate((0.0, 10.0 + h));

        // Draw via runtime effect (should still appear the same as above)
        let mut runtime_paint = Paint::default();
        canvas.draw_image(checker, (0.0, 0.0), None);
        let children = [
            ChildPtr::from(src_shader.clone()),
            ChildPtr::from(dst_shader.clone()),
            ChildPtr::from(blender.clone()),
        ];
        runtime_paint.set_shader(effect.make_shader(Data::new_empty(), &children, None));
        canvas.draw_rect(rect, &runtime_paint);
    }
}

crate::def_gm!(ArithmodeGM, ArithmodeGm);

crate::def_gm!(
    ArithmodeBlenderGM,
    ArithmodeBlenderGm {
        k1: 0.0,
        k2: 0.0,
        k3: 0.0,
        k4: 0.0,
        src: None,
        dst: None,
        checker: None,
        src_shader: None,
        dst_shader: None,
        runtime_effect: None,
    }
);
