// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rippleshadergm.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-scalar conversions of small sizes (exact in f32)
#![allow(clippy::cast_possible_truncation)] // mirrors the C++ float arithmetic of sawtoothLerp (double, then float)

use crate::GM;
use crate::tool_utils::{get_resource_as_data, get_resource_as_image};
use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::SCALAR_PI;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;

const K_SIZE: ISize = ISize::new(512, 512);

/// `fmod(fMillis, windowMs) / windowMs`, then `a * (1. - t) + b * t` in double, as the C++ does.
// Port of: gm/rippleshadergm.cpp#L37-L40 (chrome/m156), sawtoothLerp
fn sawtooth_lerp(millis: f32, a: f32, b: f32, window_ms: f32) -> f32 {
    let t = (millis % window_ms) / window_ms;
    (f64::from(a) * (1.0 - f64::from(t)) + f64::from(b) * f64::from(t)) as f32
}

// Port of: gm/rippleshadergm.cpp#L10-L62 (chrome/m156), class RippleShaderGM
struct RippleShaderGm {
    effect: Option<RuntimeEffect>,
    mandrill: Option<Shader>,
    /// `fMillis`: this allows a non-animated single-frame capture to show the effect.
    millis: f32,
}

impl GM for RippleShaderGm {
    fn name(&self) -> String {
        "rippleshader".to_string()
    }

    fn size(&mut self) -> ISize {
        K_SIZE
    }

    // Port of: gm/rippleshadergm.cpp#L13-L36 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // Load the mandrill into a shader.
        let Some(img) = get_resource_as_image("images/mandrill_512.png") else {
            eprintln!("Unable to load mandrill_512 from resources directory");
            return;
        };
        self.mandrill = img.to_shader(None, SamplingOptions::default(), None);

        // Load RippleShader.rts into a SkRuntimeEffect.
        let Some(shader_data) = get_resource_as_data("sksl/realistic/RippleShader.rts") else {
            eprintln!("Unable to load ripple shader from resources directory");
            return;
        };
        let sksl = String::from_utf8_lossy(&shader_data).into_owned();
        match RuntimeEffect::make_for_shader(sksl, None) {
            Ok(effect) => self.effect = Some(effect),
            Err(error) => eprintln!("Ripple shader failed to compile\n\n{error}"),
        }
    }

    // Port of: gm/rippleshadergm.cpp#L44-L100 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut base = Paint::default();
        base.set_shader(self.mandrill.clone());
        canvas.draw_rect(
            Rect::from_wh(K_SIZE.width as f32, K_SIZE.height as f32),
            &base,
        );

        // Uniform setting logic was imperfectly adapted from:
        //     frameworks/base/graphics/java/android/graphics/drawable/RippleShader.java
        //     frameworks/base/graphics/java/android/graphics/drawable/RippleAnimationSession.java
        let Some(effect) = self.effect.clone() else {
            return;
        };
        let mut builder = RuntimeShaderBuilder::new(effect);
        let width = K_SIZE.width as f32;
        let height = K_SIZE.height as f32;
        let anim_duration = 1500.0_f32;
        let noise_animation_duration = 7000.0_f32;
        let max_noise_phase = noise_animation_duration / 214.0_f32;
        let pi_rotate_right = SCALAR_PI * 0.007_812_5_f32;
        let pi_rotate_left = SCALAR_PI * -0.007_812_5_f32;

        let set2 = |b: &mut RuntimeShaderBuilder, name: &str, x: f32, y: f32| {
            b.uniform(name).set_f32(&[x, y]);
        };
        set2(&mut builder, "in_origin", width / 2.0, height / 2.0);
        set2(&mut builder, "in_touch", width / 2.0, height / 2.0);
        // Note that `in_progress` should actually be interpolated via FAST_OUT_SLOW_IN.
        builder.uniform("in_progress").set_f32(&[sawtooth_lerp(
            self.millis,
            0.0,
            1.0,
            anim_duration,
        )]);
        builder.uniform("in_maxRadius").set_f32(&[400.0]);
        set2(
            &mut builder,
            "in_resolutionScale",
            1.0 / width,
            1.0 / height,
        );
        set2(&mut builder, "in_noiseScale", 2.1 / width, 2.1 / height);
        builder.uniform("in_hasMask").set_f32(&[1.0]);

        let phase = sawtooth_lerp(self.millis, 0.0, max_noise_phase, noise_animation_duration);
        builder.uniform("in_noisePhase").set_f32(&[phase]);
        builder
            .uniform("in_turbulencePhase")
            .set_f32(&[phase * 1000.0]);

        let scale = 1.5_f32;
        set2(
            &mut builder,
            "in_tCircle1",
            scale * 0.5 + (phase * 0.01 * (scale * 0.55).cos()),
            scale * 0.5 + (phase * 0.01 * (scale * 0.55).sin()),
        );
        set2(
            &mut builder,
            "in_tCircle2",
            scale * 0.2 + (phase * -0.0066 * (scale * 0.45).cos()),
            scale * 0.2 + (phase * -0.0066 * (scale * 0.45).sin()),
        );
        set2(
            &mut builder,
            "in_tCircle3",
            scale + (phase * -0.0066 * (scale * 0.35).cos()),
            scale + (phase * -0.0066 * (scale * 0.35).sin()),
        );

        let rotation1 = phase * pi_rotate_right + 1.7 * SCALAR_PI;
        set2(
            &mut builder,
            "in_tRotation1",
            rotation1.cos(),
            rotation1.sin(),
        );
        let rotation2 = phase * pi_rotate_left + 2.0 * SCALAR_PI;
        set2(
            &mut builder,
            "in_tRotation2",
            rotation2.cos(),
            rotation2.sin(),
        );
        let rotation3 = phase * pi_rotate_right + 2.75 * SCALAR_PI;
        set2(
            &mut builder,
            "in_tRotation3",
            rotation3.cos(),
            rotation3.sin(),
        );

        builder.uniform("in_color").set_f32(&[0.0, 0.6, 0.0, 1.0]); // green
        builder
            .uniform("in_sparkleColor")
            .set_f32(&[1.0, 1.0, 1.0, 1.0]); // white

        builder
            .child("in_shader")
            .assign(self.mandrill.clone().expect("the mandrill shader"));

        let mut sparkle = Paint::default();
        sparkle.set_shader(builder.make_shader(None));
        canvas.draw_rect(Rect::from_wh(width, height), &sparkle);
    }
}

// Port of: gm/rippleshadergm.cpp#L62 (chrome/m156), DEF_GM(return new RippleShaderGM;)
crate::def_gm!(
    RippleShaderGM,
    RippleShaderGm {
        effect: None,
        mandrill: None,
        millis: 500.0,
    }
);
