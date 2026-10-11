// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/shaders/SkGainmapShader.cpp and include/private/SkGainmapShader.h (chrome/m156).
//
// The SkSL is copied verbatim from the C++ source (as the same string literals, joined by
// `concat!`). The uniforms are computed with the same float operations, in the same order.

use std::sync::OnceLock;

use skia_rust_core::color::{Color4f, ColorChannelFlag};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::gainmap_info::{BaseImageType, GainmapInfo, GainmapType};
use skia_rust_core::image::Image;
use skia_rust_core::image_info_priv::color_type_channel_flags;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeEffectBuilder};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;

// Port of: src/shaders/SkGainmapShader.cpp#L20-L77 (chrome/m156), `gGainmapSKSL`.
const GAINMAP_SKSL: &str = concat!(
    "uniform shader base;",
    "uniform shader gainmap;",
    "uniform half4 logRatioMin;",
    "uniform half4 logRatioMax;",
    "uniform half4 gainmapGamma;",
    "uniform half4 epsilonBase;",
    "uniform half4 epsilonOther;",
    "uniform half W;",
    "uniform int gainmapIsAlpha;",
    "uniform int gainmapIsRed;",
    "uniform int singleChannel;",
    "uniform int noGamma;",
    "uniform int isApple;",
    "uniform half appleG;",
    "uniform half appleH;",
    "",
    "half4 main(float2 coord) {",
    "half4 S = base.eval(coord);",
    "half4 G = gainmap.eval(coord);",
    "if (gainmapIsAlpha == 1) {",
    "G = half4(G.a, G.a, G.a, 1.0);",
    "}",
    "if (gainmapIsRed == 1) {",
    "G = half4(G.r, G.r, G.r, 1.0);",
    "}",
    "if (singleChannel == 1) {",
    "half L;",
    "if (isApple == 1) {",
    "L = pow(G.r, appleG);",
    "L = log(1.0 + (appleH - 1.0) * pow(G.r, appleG));",
    "} else if (noGamma == 1) {",
    "L = mix(logRatioMin.r, logRatioMax.r, G.r);",
    "} else {",
    "L = mix(logRatioMin.r, logRatioMax.r, pow(G.r, gainmapGamma.r));",
    "}",
    "half3 H = (S.rgb + epsilonBase.rgb) * exp(L * W) - epsilonOther.rgb;",
    "return half4(H.r, H.g, H.b, S.a);",
    "} else {",
    "half3 L;",
    "if (isApple == 1) {",
    "L = pow(G.rgb, half3(appleG));",
    "L = log(half3(1.0) + (appleH - 1.0) * L);",
    "} else if (noGamma == 1) {",
    "L = mix(logRatioMin.rgb, logRatioMax.rgb, G.rgb);",
    "} else {",
    "L = mix(logRatioMin.rgb, logRatioMax.rgb, pow(G.rgb, gainmapGamma.rgb));",
    "}",
    "half3 H = (S.rgb + epsilonBase.rgb) * exp(L * W) - epsilonOther.rgb;",
    "return half4(H.r, H.g, H.b, S.a);",
    "}",
    "}",
);

// Port of: src/shaders/SkGainmapShader.cpp#L79-L86 (chrome/m156), `gainmap_apply_effect`.
fn gainmap_apply_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    EFFECT
        .get_or_init(|| {
            RuntimeEffect::make_for_shader(GAINMAP_SKSL, None)
                .expect("the gainmap SkSL is a valid shader")
        })
        .clone()
}

// Port of: src/shaders/SkGainmapShader.cpp#L88-L90 (chrome/m156), `all_channels_equal`.
#[allow(clippy::float_cmp)] // exact comparison, as in C++
fn all_channels_equal(c: Color4f) -> bool {
    c.r == c.g && c.r == c.b
}

// The four floats of an SkColor4f, in the order the uniform stores them.
fn color4f_components(c: Color4f) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

/// Gainmap shader factory (`SkGainmapShader`).
///
/// When sampling the base image `base_image`, `base_rect` maps to `dst_rect`, sampled with
/// `base_sampling_options`. When sampling the gainmap image, `gainmap_rect` maps to `dst_rect`,
/// sampled with `gainmap_sampling_options`. The gainmap is applied according to the HDR to SDR
/// ratio in `dst_hdr_ratio`.
#[doc(alias = "SkGainmapShader")]
#[derive(Debug)]
pub struct GainmapShader;

impl GainmapShader {
    /// Make a gainmap shader (`SkGainmapShader::Make`).
    // Port of: src/shaders/SkGainmapShader.cpp#L131-L209 (chrome/m156), the 9-argument `Make`.
    #[doc(alias = "Make")]
    #[must_use]
    #[allow(
        clippy::too_many_arguments, // mirrors SkGainmapShader::Make
        clippy::too_many_lines,     // one function, as in C++
        clippy::float_cmp           // `w == 0.f` and the gamma == 1 tests are exact, as in C++
    )]
    pub fn make(
        base_image: &Image,
        base_rect: &Rect,
        base_sampling_options: SamplingOptions,
        gainmap_image: &Image,
        gainmap_rect: &Rect,
        gainmap_sampling_options: SamplingOptions,
        gainmap_info: &GainmapInfo,
        dst_rect: &Rect,
        dst_hdr_ratio: f32,
    ) -> Option<Shader> {
        let base_color_space = base_image
            .color_space()
            .unwrap_or_else(ColorSpace::new_srgb);

        // Determine the color space in which the gainmap math is to be applied.
        let gainmap_math_color_space = match &gainmap_info.gainmap_math_color_space {
            Some(cs) => cs.with_linear_gamma(),
            None => base_color_space.with_linear_gamma(),
        };

        // Compute the sampling transformation matrices.
        let base_rect_to_dst_rect = Matrix::rect_to_rect_or_identity(base_rect, dst_rect, None);
        let gainmap_rect_to_dst_rect =
            Matrix::rect_to_rect_or_identity(gainmap_rect, dst_rect, None);

        // Compute the weight parameter that will be used to blend between the images.
        let mut w: f32 = 0.0;
        if dst_hdr_ratio > gainmap_info.display_ratio_sdr {
            if dst_hdr_ratio < gainmap_info.display_ratio_hdr {
                w = (dst_hdr_ratio.ln() - gainmap_info.display_ratio_sdr.ln())
                    / (gainmap_info.display_ratio_hdr.ln() - gainmap_info.display_ratio_sdr.ln());
            } else {
                w = 1.0;
            }
        }

        let base_image_is_hdr = gainmap_info.base_image_type == BaseImageType::Hdr;
        if base_image_is_hdr {
            w -= 1.0;
        }

        // Return the base image directly if the gainmap will not be applied at all.
        if w == 0.0 {
            return base_image.to_shader(None, base_sampling_options, &base_rect_to_dst_rect);
        }

        // The base image will have color space conversion performed.
        let base_image_shader =
            base_image.to_shader(None, base_sampling_options, &base_rect_to_dst_rect)?;

        // The gainmap image shader will ignore any color space that the gainmap has.
        let gainmap_image_shader = gainmap_image.to_raw_shader(
            None,
            gainmap_sampling_options,
            &gainmap_rect_to_dst_rect,
        )?;

        // Create the shader to apply the gainmap in the gain application color space.
        let gainmap_math_shader = {
            let mut builder = RuntimeEffectBuilder::new(gainmap_apply_effect());
            let gamma = gainmap_info.gainmap_gamma;
            let ratio_min = gainmap_info.gainmap_ratio_min;
            let ratio_max = gainmap_info.gainmap_ratio_max;
            let log_ratio_min = Color4f {
                r: ratio_min.r.ln(),
                g: ratio_min.g.ln(),
                b: ratio_min.b.ln(),
                a: 1.0,
            };
            let log_ratio_max = Color4f {
                r: ratio_max.r.ln(),
                g: ratio_max.g.ln(),
                b: ratio_max.b.ln(),
                a: 1.0,
            };
            let no_gamma = gamma.r == 1.0 && gamma.g == 1.0 && gamma.b == 1.0;
            let color_type_flags = color_type_channel_flags(gainmap_image.color_type());
            let gainmap_is_alpha = color_type_flags == ColorChannelFlag::ALPHA;
            let gainmap_is_red = color_type_flags == ColorChannelFlag::RED;
            let single_channel = all_channels_equal(gamma)
                && all_channels_equal(ratio_min)
                && all_channels_equal(ratio_max)
                && (color_type_flags == ColorChannelFlag::GRAY
                    || color_type_flags == ColorChannelFlag::ALPHA
                    || color_type_flags == ColorChannelFlag::RED);
            let epsilon_base = if base_image_is_hdr {
                gainmap_info.epsilon_hdr
            } else {
                gainmap_info.epsilon_sdr
            };
            let epsilon_other = if base_image_is_hdr {
                gainmap_info.epsilon_sdr
            } else {
                gainmap_info.epsilon_hdr
            };
            let is_apple = gainmap_info.gainmap_type == GainmapType::Apple;
            let apple_g: f32 = 1.961;
            let apple_h = gainmap_info.display_ratio_hdr;

            // A failed assignment means the effect lacks the variable, which it does not.
            let _ = builder.child("base").assign(base_image_shader);
            let _ = builder.child("gainmap").assign(gainmap_image_shader);
            let _ = builder
                .uniform("logRatioMin")
                .set_f32(&color4f_components(log_ratio_min));
            let _ = builder
                .uniform("logRatioMax")
                .set_f32(&color4f_components(log_ratio_max));
            let _ = builder
                .uniform("gainmapGamma")
                .set_f32(&color4f_components(gamma));
            let _ = builder
                .uniform("epsilonBase")
                .set_f32(&color4f_components(epsilon_base));
            let _ = builder
                .uniform("epsilonOther")
                .set_f32(&color4f_components(epsilon_other));
            let _ = builder.uniform("noGamma").set_i32(&[i32::from(no_gamma)]);
            let _ = builder
                .uniform("singleChannel")
                .set_i32(&[i32::from(single_channel)]);
            let _ = builder
                .uniform("gainmapIsAlpha")
                .set_i32(&[i32::from(gainmap_is_alpha)]);
            let _ = builder
                .uniform("gainmapIsRed")
                .set_i32(&[i32::from(gainmap_is_red)]);
            let _ = builder.uniform("W").set_f32(&[w]);
            let _ = builder.uniform("isApple").set_i32(&[i32::from(is_apple)]);
            let _ = builder.uniform("appleG").set_f32(&[apple_g]);
            let _ = builder.uniform("appleH").set_f32(&[apple_h]);
            builder.make_shader(None)?
        };
        Some(gainmap_math_shader.with_working_color_space(gainmap_math_color_space, None))
    }

    /// Make a gainmap shader, with the destination color space (`SkGainmapShader::Make` with
    /// `dstColorSpace`). As in C++, the color space is not used.
    // Port of: src/shaders/SkGainmapShader.cpp#L114-L129 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    #[allow(clippy::too_many_arguments)] // mirrors SkGainmapShader::Make
    pub fn make_with_dst_color_space(
        base_image: &Image,
        base_rect: &Rect,
        base_sampling_options: SamplingOptions,
        gainmap_image: &Image,
        gainmap_rect: &Rect,
        gainmap_sampling_options: SamplingOptions,
        gainmap_info: &GainmapInfo,
        dst_rect: &Rect,
        dst_hdr_ratio: f32,
        _dst_color_space: Option<ColorSpace>,
    ) -> Option<Shader> {
        Self::make(
            base_image,
            base_rect,
            base_sampling_options,
            gainmap_image,
            gainmap_rect,
            gainmap_sampling_options,
            gainmap_info,
            dst_rect,
            dst_hdr_ratio,
        )
    }
}
