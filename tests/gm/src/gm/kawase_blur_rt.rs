// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/kawase_blur_rt.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-float conversions of small sizes (exact in f32)
#![allow(clippy::cast_possible_truncation)] // mirrors the C++ float-to-int conversions (values are exact and in range)
#![allow(clippy::cast_sign_loss)] // mirrors the C++ (uint32_t) cast of a non-negative ceil

use crate::GM;
use crate::canvas::{Canvas, Surface};
use crate::tool_utils::get_resource_as_image;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::runtime_effect::ChildPtr;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;

/// `KawaseBlurFilter::kInputScale`.
// Port of: gm/kawase_blur_rt.cpp#L26-L26 (chrome/m156)
const K_INPUT_SCALE: f32 = 0.25;
/// `KawaseBlurFilter::kInverseInputScale`.
// Port of: gm/kawase_blur_rt.cpp#L28-L28 (chrome/m156)
const K_INVERSE_INPUT_SCALE: f32 = 1.0 / K_INPUT_SCALE;
/// `KawaseBlurFilter::kMaxPasses`.
// Port of: gm/kawase_blur_rt.cpp#L30-L30 (chrome/m156)
const K_MAX_PASSES: u32 = 4;
/// `KawaseBlurFilter::kMaxCrossFadeRadius`.
// Port of: gm/kawase_blur_rt.cpp#L33-L33 (chrome/m156)
const K_MAX_CROSS_FADE_RADIUS: f32 = 30.0;

/// `builder.child(name) = shader` for a possibly null `sk_sp<SkShader>`.
fn set_child(builder: &mut RuntimeShaderBuilder, name: &str, shader: Option<Shader>) {
    match shader {
        Some(shader) => {
            builder.child(name).assign(ChildPtr::from(shader));
        }
        None => {
            builder.child(name).assign_null();
        }
    }
}

/// `SkSamplingOptions(SkFilterMode::kLinear)`.
fn linear() -> SamplingOptions {
    SamplingOptions::from(FilterMode::Linear)
}

/// `KawaseBlurFilter::MakeSurface`. `canvas->makeSurface(info)` on the raster sink yields a raster
/// surface with the same info, which is what the fallback `SkSurfaces::Raster(info)` creates.
// Port of: gm/kawase_blur_rt.cpp#L77-L84 (chrome/m156)
fn make_surface(info: &ImageInfo) -> Surface<'static> {
    skia_rust_raster::surfaces::raster(info, None, None)
        .expect("kawase_blur_rt: raster surface for a valid N32 info")
}

// Port of: gm/kawase_blur_rt.cpp#L14-L75 (chrome/m156), class KawaseBlurFilter
struct KawaseBlurFilter {
    blur_effect: RuntimeEffect,
    mix_effect: RuntimeEffect,
}

impl KawaseBlurFilter {
    // Port of: gm/kawase_blur_rt.cpp#L36-L75 (chrome/m156), KawaseBlurFilter()
    fn new() -> Self {
        let blur_string = r"
            uniform shader src;
            uniform float in_inverseScale;
            uniform float2 in_blurOffset;

            half4 main(float2 xy) {
                float2 scaled_xy = float2(xy.x * in_inverseScale, xy.y * in_inverseScale);

                half4 c = src.eval(scaled_xy);
                c += src.eval(scaled_xy + float2( in_blurOffset.x,  in_blurOffset.y));
                c += src.eval(scaled_xy + float2( in_blurOffset.x, -in_blurOffset.y));
                c += src.eval(scaled_xy + float2(-in_blurOffset.x,  in_blurOffset.y));
                c += src.eval(scaled_xy + float2(-in_blurOffset.x, -in_blurOffset.y));

                return half4(c.rgb * 0.2, 1.0);
            }
        ";
        let mix_string = r"
            uniform shader in_blur;
            uniform shader in_original;
            uniform float in_inverseScale;
            uniform float in_mix;

            half4 main(float2 xy) {
                float2 scaled_xy = float2(xy.x * in_inverseScale, xy.y * in_inverseScale);

                half4 blurred = in_blur.eval(scaled_xy);
                half4 composition = in_original.eval(xy);
                return mix(composition, blurred, in_mix);
            }
        ";
        let blur_effect = RuntimeEffect::make_for_shader(blur_string, None)
            .expect("kawase_blur_rt: blur effect compiles");
        let mix_effect = RuntimeEffect::make_for_shader(mix_string, None)
            .expect("kawase_blur_rt: mix effect compiles");
        Self {
            blur_effect,
            mix_effect,
        }
    }

    // Port of: gm/kawase_blur_rt.cpp#L86-L162 (chrome/m156), KawaseBlurFilter::draw
    fn draw(&self, canvas: &Canvas, input: &Image, blur_radius: i32) {
        // Kawase is an approximation of Gaussian, but it behaves differently from it.
        // A radius transformation is required for approximating them, and also to introduce
        // non-integer steps, necessary to smoothly interpolate large radii.
        let tmp_radius = blur_radius as f32 / 6.0_f32;
        let number_of_passes = K_MAX_PASSES.min(tmp_radius.ceil() as u32) as f32;
        let radius_by_passes = tmp_radius / number_of_passes;

        // The C++ `SkImageInfo::MakeN32Premul(float, float)` converts to int (exact here).
        let scaled_info = ImageInfo::new_n32_premul(
            ISize::new(
                (input.width() as f32 * K_INPUT_SCALE) as i32,
                (input.height() as f32 * K_INPUT_SCALE) as i32,
            ),
            None,
        );
        let mut draw_surface = make_surface(&scaled_info);

        let step_x = radius_by_passes;
        let step_y = radius_by_passes;

        // start by drawing and downscaling and doing the first blur pass
        let mut blur_builder = RuntimeShaderBuilder::new(self.blur_effect.clone());
        set_child(
            &mut blur_builder,
            "src",
            input.to_shader(None, linear(), None),
        );
        blur_builder
            .uniform("in_inverseScale")
            .set_f32(&[K_INVERSE_INPUT_SCALE]);
        blur_builder.uniform("in_blurOffset").set_f32(&[
            step_x * K_INVERSE_INPUT_SCALE,
            step_y * K_INVERSE_INPUT_SCALE,
        ]);
        let mut paint = Paint::default();
        paint.set_shader(blur_builder.make_shader(None));
        draw_surface
            .canvas()
            .draw_irect(scaled_info.bounds(), &paint);

        // DEBUG draw each of the stages
        canvas.save();
        if let Some(snapshot) = draw_surface.image_snapshot() {
            canvas.draw_image(snapshot, ((input.width() / 4) as f32, 0.0), None);
        }
        canvas.translate(((input.width() / 4) as f32, input.height() as f32 * 0.75));

        // And now we'll ping pong between our surfaces, to accumulate the result of various
        // offsets.
        let last_draw_target = if number_of_passes > 1.0 {
            let mut read_surface = draw_surface;
            draw_surface = make_surface(&scaled_info);

            let mut i = 1_i32;
            while (i as f32) < number_of_passes {
                let step_scale = i as f32 * K_INPUT_SCALE;

                let read_snapshot = read_surface.image_snapshot();
                set_child(
                    &mut blur_builder,
                    "src",
                    read_snapshot.and_then(|s| s.to_shader(None, linear(), None)),
                );
                blur_builder.uniform("in_inverseScale").set_f32(&[1.0]);
                blur_builder
                    .uniform("in_blurOffset")
                    .set_f32(&[step_x * step_scale, step_y * step_scale]);

                paint.set_shader(blur_builder.make_shader(None));
                draw_surface
                    .canvas()
                    .draw_irect(scaled_info.bounds(), &paint);

                // DEBUG draw each of the stages
                if let Some(snapshot) = draw_surface.image_snapshot() {
                    canvas.draw_image(snapshot, (0.0, 0.0), None);
                }
                canvas.translate((0.0, input.height() as f32 * 0.75));

                // Swap buffers for next iteration
                std::mem::swap(&mut draw_surface, &mut read_surface);
                i += 1;
            }
            read_surface
        } else {
            draw_surface
        };

        // restore translations done for debug and offset
        canvas.restore();
        let _acr = AutoCanvasRestore::guard(canvas, true);
        canvas.translate((input.width() as f32, 0.0));

        // do the final composition and when we scale our blur up. It will be interpolated
        // with the larger composited texture to hide downscaling artifacts.
        let mut mix_builder = RuntimeShaderBuilder::new(self.mix_effect.clone());
        let mut last_target = last_draw_target;
        let blur_snapshot = last_target.image_snapshot();
        set_child(
            &mut mix_builder,
            "in_blur",
            blur_snapshot.and_then(|s| s.to_shader(None, linear(), None)),
        );
        set_child(
            &mut mix_builder,
            "in_original",
            input.to_shader(None, linear(), None),
        );
        mix_builder
            .uniform("in_inverseScale")
            .set_f32(&[K_INPUT_SCALE]);
        mix_builder
            .uniform("in_mix")
            .set_f32(&[1.0_f32.min(blur_radius as f32 / K_MAX_CROSS_FADE_RADIUS)]);

        paint.set_shader(mix_builder.make_shader(None));
        canvas.draw_irect(input.bounds(), &paint);
    }
}

// Port of: gm/kawase_blur_rt.cpp#L164-L196 (chrome/m156), class KawaseBlurRT
struct KawaseBlurRtGm {
    mandrill: Option<Image>,
}

impl GM for KawaseBlurRtGm {
    // Port of: gm/kawase_blur_rt.cpp#L186-L186 (chrome/m156), getName
    fn name(&self) -> String {
        "kawase_blur_rt".to_string()
    }

    // Port of: gm/kawase_blur_rt.cpp#L187-L187 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1280, 768)
    }

    // Port of: gm/kawase_blur_rt.cpp#L189-L192 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.mandrill = get_resource_as_image("images/mandrill_256.png");
    }

    // Port of: gm/kawase_blur_rt.cpp#L194-L204 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(mandrill) = self.mandrill.clone() else {
            return;
        };
        canvas.draw_image(&mandrill, (0.0, 0.0), None);
        canvas.translate((256.0, 0.0));
        let blur_filter = KawaseBlurFilter::new();
        blur_filter.draw(canvas, &mandrill, 45);
        canvas.translate((512.0, 0.0));
        blur_filter.draw(canvas, &mandrill, 55);
    }
}

// Port of: gm/kawase_blur_rt.cpp#L205-L205 (chrome/m156), DEF_GM(return new KawaseBlurRT;)
crate::def_gm!(KawaseBlurRT, KawaseBlurRtGm { mandrill: None });
