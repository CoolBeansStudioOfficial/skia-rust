// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkHdrAgtm.cpp#L24-L140 (the `gAgtmSKSL` shader source and its runtime effect),
// #L498-L545 (MakeColorFilter, MakeGainCurveXYMImage) and the `skhdr::Metadata::makeToneMapColorFilter`
// of src/codec/SkHdrMetadata.cpp (chrome/m156): the AGTM tone map as a colour filter.

//! The adaptive global tone map as an `SkColorFilter` (`makeToneMapColorFilter`).

// The dimensions and uniform values are small integers and counts, converted to `f32` as the C++
// does for the SkSL uniforms.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use crate::alpha_type::AlphaType;
use crate::color_filter::ColorFilter;
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::data::Data;
use crate::half::float_to_half;
use crate::hdr_agtm::{HeadroomAdaptiveToneMap, Metadata};
use crate::hdr_agtm_helpers::{
    compute_weighting, get_gain_application_space, populate_tone_map_agtm_params,
};
use crate::image::Image;
use crate::image_info::ImageInfo;
use crate::images;
use crate::runtime_effect::{RuntimeEffect, RuntimeShaderBuilder};
use crate::sampling_options::FilterMode;

// Port of: src/codec/SkHdrAgtm.cpp#L24-L140 (chrome/m156), gAgtmSKSL
const AGTM_SKSL: &str = concat!(
    "uniform half scale_factor;",       // The scale to apply in linear space
    "uniform shader curve_xym;",        // The texture containing control points.
    "uniform half weight_i;",           // The weight of gain curve "i"
    "uniform half4 mix_rgbx_i;",        // The red,green,blue mixing coefficients.
    "uniform half4 mix_Mmcx_i;",        // The max,min,component mixing coefficients.
    "uniform half curve_texcoord_y_i;", // The y texture coordinate at which to sample curve_xym.
    "uniform half curve_N_cp_i;",       // The number of control points.
    "uniform half weight_j;",           // All the same parameters, for gain curve "j"
    "uniform half4 mix_rgbx_j;",
    "uniform half4 mix_Mmcx_j;",
    "uniform half curve_texcoord_y_j;",
    "uniform half curve_N_cp_j;",
    // Shader equivalent of AgtmHelpers::EvaluateComponentMixingFunction.
    "half3 EvalComponentMixing(half3 color, half4 rgbx, half4 Mmcx) {",
    "half common = dot(rgbx.rgb, color) +",
    "Mmcx[0] * max(max(color.r, color.g), color.b) +",
    "Mmcx[1] * min(min(color.r, color.g), color.b);",
    "return Mmcx[2] * color + half3(common);",
    "}",
    // Shader equivalent of AgtmHelpers::EvaluateGainCurve.
    "half EvalGainCurve(half x, half curve_texcoord_y, half curve_N_cp) {",
    // Handle points to the left of the first control point.
    "half c_min = 0.0;",
    "half4 xym_min = curve_xym.eval(half2(c_min + 0.5, curve_texcoord_y));",
    "if (x <= xym_min.x) {",
    "return xym_min.y;",
    "}",
    // Handle points after the last control point.
    "half c_max = curve_N_cp - 1.0;",
    "half4 xym_max = curve_xym.eval(half2(c_max + 0.5, curve_texcoord_y));",
    "if (x >= xym_max.x) {",
    "return xym_max.y + log2(xym_max.x / x);",
    "}",
    // Binary search for the interval containing x. This will require sampling at most
    // log2(32)=5 more control points.
    "for (int step = 0; step < 5; ++step) {",
    // Early-out if we've already found the interval.
    "if (c_max - c_min <= 1.0) {",
    "break;",
    "}",
    // Test the midpoint and replace one of the endpoints with it.
    "half c_mid = ceil(0.5 * (c_min + c_max));",
    "half4 xym_mid = curve_xym.eval(half2(c_mid + 0.5, curve_texcoord_y));",
    "if (x == xym_mid.x) {",
    // If we hit a control point exactly, then just return.
    "return xym_mid.y;",
    "} else if (x < xym_mid.x) {",
    "c_max = c_mid;",
    "xym_max = xym_mid;",
    "} else {",
    "c_min = c_mid;",
    "xym_min = xym_mid;",
    "}",
    "}",
    // Evaluate the cubic.
    "half h = xym_max.x - xym_min.x;",
    "half mHat_min = xym_min.z * h;",
    "half mHat_max = xym_max.z * h;",
    "half c3 =  2.0 * xym_min.y + mHat_min - 2.0 * xym_max.y + mHat_max;",
    "half c2 = -3.0 * xym_min.y + 3.0 * xym_max.y - 2.0 * mHat_min - mHat_max;",
    "half c1 = mHat_min;",
    "half c0 = xym_min.y;",
    "half t = (x - xym_min.x) / h;",
    "return ((c3*t + c2)*t + c1)*t + c0;",
    "}",
    // Shader equivalent of AgtmHelpers::EvaluateColorGainFunction.
    "half3 EvalColorGainFunction(half3 color,",
    "half4 mix_rgbx, half4 mix_Mmcx,",
    "float curve_texcoord_y, float curve_N_cp) {",
    "half3 M = EvalComponentMixing(color, mix_rgbx, mix_Mmcx);",
    "if (mix_Mmcx.b == 0.0) {",
    // If the kComponent coefficient is zero, only evalute the curve once.
    "return half3(EvalGainCurve(M.r, curve_texcoord_y, curve_N_cp));",
    "}",
    "return half3(EvalGainCurve(M.r, curve_texcoord_y, curve_N_cp),",
    "EvalGainCurve(M.g, curve_texcoord_y, curve_N_cp),",
    "EvalGainCurve(M.b, curve_texcoord_y, curve_N_cp));",
    "}",
    // Shader equivalent of AgtmHelpers::ApplyGain.
    "half4 main(half4 color) {",
    "color.rgb *= scale_factor;",
    "if (weight_i > 0.0) {",
    // Unpremultiply alpha is needed.
    "float a_inv = (color.a == 0.0) ? 1.0 : 1.0 / color.a;",
    "half3 G = half3(0.0);",
    "G += weight_i * EvalColorGainFunction(color.rgb * a_inv,",
    "mix_rgbx_i, mix_Mmcx_i,",
    "curve_texcoord_y_i, curve_N_cp_i);",
    "if (weight_j > 0.0) {",
    "G += weight_j * EvalColorGainFunction(color.rgb * a_inv,",
    "mix_rgbx_j, mix_Mmcx_j,",
    "curve_texcoord_y_j, curve_N_cp_j);",
    "}",
    "color.rgb *= exp2(G);",
    "}",
    "return color;",
    "}",
);

/// The control points of all alternate images as an F16 image: the X, Y and M values of each control
/// point in the colors of the rows, one row per alternate image (`MakeGainCurveXYMImage`).
// Port of: src/codec/SkHdrAgtm.cpp#L142-L233 (chrome/m156), MakeGainCurveXYMImage
#[doc(alias = "MakeGainCurveXYMImage")]
fn make_gain_curve_xym_image(hatm: &HeadroomAdaptiveToneMap) -> Option<Image> {
    // Four half-precision channels per pixel, one row per alternate image.
    const BYTES_PER_PIXEL: usize = 8;
    if hatm.alternate_images.is_empty() {
        return None;
    }
    let max_num_control_points = hatm
        .alternate_images
        .iter()
        .map(|alt| alt.color_gain_function.gain_curve.control_points.len())
        .fold(1, usize::max);

    let row_bytes = max_num_control_points * BYTES_PER_PIXEL;
    let mut pixels = vec![0u8; row_bytes * hatm.alternate_images.len()];
    for (a, alt) in hatm.alternate_images.iter().enumerate() {
        for (c, cp) in alt
            .color_gain_function
            .gain_curve
            .control_points
            .iter()
            .enumerate()
        {
            // Convert from F32 to F16 for use on the GPU.
            let values = [cp.x, cp.y, cp.m, 1.0];
            for (k, value) in values.into_iter().enumerate() {
                let offset = a * row_bytes + c * BYTES_PER_PIXEL + k * 2;
                pixels[offset..offset + 2].copy_from_slice(&float_to_half(value).to_ne_bytes());
            }
        }
    }

    // Width is the number of control points, height is the number of alternate images.
    #[allow(clippy::cast_possible_truncation)] // the dimensions are small
    let dimensions = (
        max_num_control_points as i32,
        hatm.alternate_images.len() as i32,
    );
    let info = ImageInfo::new(dimensions, ColorType::RGBAF16, AlphaType::Premul, None);
    images::raster_from_data(&info, Data::new_from_vec(pixels), row_bytes)
}

/// Returns an `SkColorFilter` that will first scale by `scale_factor`, and then tone map to the
/// specified `targeted_hdr_headroom` (`MakeColorFilter`).
// Port of: src/codec/SkHdrAgtm.cpp#L498-L545 (chrome/m156), MakeColorFilter
#[doc(alias = "MakeColorFilter")]
#[must_use]
pub fn make_color_filter(
    hatm: &HeadroomAdaptiveToneMap,
    targeted_hdr_headroom: f32,
    scale_factor: f32,
) -> Option<ColorFilter> {
    const WEIGHT_NAMES: [&str; 2] = ["weight_i", "weight_j"];
    const MIX_RGBX_NAMES: [&str; 2] = ["mix_rgbx_i", "mix_rgbx_j"];
    const MIX_MMCX_NAMES: [&str; 2] = ["mix_Mmcx_i", "mix_Mmcx_j"];
    const CURVE_TEXCOORD_Y_NAMES: [&str; 2] = ["curve_texcoord_y_i", "curve_texcoord_y_j"];
    const CURVE_N_CP_NAMES: [&str; 2] = ["curve_N_cp_i", "curve_N_cp_j"];

    let weighting = compute_weighting(hatm, targeted_hdr_headroom);

    let effect = RuntimeEffect::make_for_color_filter(AGTM_SKSL, None).ok()?;
    let mut builder = RuntimeShaderBuilder::new(effect);
    builder.uniform("scale_factor").set_f32(&[scale_factor]);
    for a in 0..2 {
        builder
            .uniform(WEIGHT_NAMES[a])
            .set_f32(&[weighting.weight[a]]);

        if weighting.weight[a] == 0.0 {
            continue;
        }
        let index = usize::from(weighting.alternate_image_index[a]);
        let gain = &hatm.alternate_images[index].color_gain_function;
        let mix = &gain.component_mixing;

        builder
            .uniform(MIX_RGBX_NAMES[a])
            .set_f32(&[mix.red, mix.green, mix.blue, 0.0]);
        builder
            .uniform(MIX_MMCX_NAMES[a])
            .set_f32(&[mix.max, mix.min, mix.component, 0.0]);
        builder
            .uniform(CURVE_TEXCOORD_Y_NAMES[a])
            .set_f32(&[f32::from(weighting.alternate_image_index[a]) + 0.5]);
        builder
            .uniform(CURVE_N_CP_NAMES[a])
            .set_f32(&[gain.gain_curve.control_points.len() as f32]);
    }

    if let Some(gain_curves_xym) = make_gain_curve_xym_image(hatm)
        && let Some(shader) = gain_curves_xym.to_raw_shader(
            None::<(crate::tile_mode::TileMode, crate::tile_mode::TileMode)>,
            FilterMode::Nearest,
            None::<&crate::matrix::Matrix>,
        )
    {
        builder.child("curve_xym").assign(shader);
    }

    let gain_application_color_space = get_gain_application_space(hatm)?;
    let filter = builder.make_color_filter()?;
    filter.with_working_color_space(gain_application_color_space)
}

impl Metadata {
    /// Returns the colour filter that tone maps to the specified targeted HDR headroom
    /// (`makeToneMapColorFilter`). If `input_color_space` is PQ or HLG, the filter reinterprets the
    /// image as having the HDR reference white of the metadata.
    // Port of: src/codec/SkHdrMetadata.cpp#L262-L276 (chrome/m156), Metadata::makeToneMapColorFilter
    #[must_use]
    #[doc(alias = "makeToneMapColorFilter")]
    pub fn make_tone_map_color_filter(
        &self,
        targeted_hdr_headroom: f32,
        input_color_space: Option<&ColorSpace>,
    ) -> Option<ColorFilter> {
        let (agtm, scale_factor) = populate_tone_map_agtm_params(self, input_color_space)?;
        // TODO(https://crbug.com/395659818): Add default tone mapping.
        let hatm = agtm.headroom_adaptive_tone_map?;
        make_color_filter(&hatm, targeted_hdr_headroom, scale_factor)
    }
}
