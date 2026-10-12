// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/private/SkHdrMetadata.h#L381-L470 and src/codec/SkHdrAgtm.cpp (chrome/m156):
// the `skhdr::AgtmHelpers` functions that evaluate, populate, validate and apply the adaptive
// global tone map.

//! Helpers for the adaptive global tone map (`skhdr::AgtmHelpers`).

// The ported arithmetic keeps the C++ evaluation order and exact float comparisons, and the
// single-letter names of the Skia formulas (`c`, `h`, `s`, `x`, `y`, `m`, ...) are kept so each
// function can be checked against its source line by line.
#![allow(
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::if_not_else
)]

use crate::color::Color4f;
use crate::color_space::{ColorSpace, named_primaries, named_transfer_fn};
use crate::hdr_agtm::{
    AdaptiveGlobalToneMap, AlternateImage, ColorGainFunction, ComponentMixingFunction, GainCurve,
    GainCurveControlPoint, HeadroomAdaptiveToneMap, Metadata,
};

/// Port of `std::max(a, b)`: `a < b ? b : a`.
// Port of: the std::max used throughout src/codec/SkHdrAgtm.cpp (chrome/m156)
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// Port of `std::min(a, b)`: `b < a ? b : a`.
// Port of: the std::min used throughout src/codec/SkHdrAgtm.cpp (chrome/m156)
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// Port of `std::signbit`.
fn signbit(x: f32) -> bool {
    x.is_sign_negative()
}

/// The function evaluation described in Clause 6.4.3 (`EvaluateComponentMixingFunction`).
// Port of: src/codec/SkHdrAgtm.cpp#L63-L80 (chrome/m156), EvaluateComponentMixingFunction
#[doc(alias = "EvaluateComponentMixingFunction")]
#[must_use]
pub fn evaluate_component_mixing_function(mix: &ComponentMixingFunction, c: &Color4f) -> Color4f {
    // This implements that math in Formula (9).
    let common = mix.red * c.r
        + mix.green * c.g
        + mix.blue * c.b
        + mix.max * std_max(std_max(c.r, c.g), c.b)
        + mix.min * std_min(std_max(c.r, c.g), c.b);

    // Optimization for when all components are the same.
    if mix.component == 0.0 {
        return Color4f {
            r: common,
            g: common,
            b: common,
            a: c.a,
        };
    }

    // Formula (10).
    Color4f {
        r: mix.component * c.r + common,
        g: mix.component * c.g + common,
        b: mix.component * c.b + common,
        a: c.a,
    }
}

/// The function evaluation described in Clause 6.5.3 (`EvaluateGainCurve`).
// Port of: src/codec/SkHdrAgtm.cpp#L82-L126 (chrome/m156), EvaluateGainCurve
#[doc(alias = "EvaluateGainCurve")]
#[must_use]
pub fn evaluate_gain_curve(gain_curve: &GainCurve, x: f32) -> f32 {
    let cp = &gain_curve.control_points;
    let n = cp.len();

    // This implements that math in Formula (11).
    debug_assert!(n > 0 && n <= GainCurve::MAX_NUM_CONTROL_POINTS);

    // Handle points off of the left endpoint.
    let mut i = 0usize;
    if x <= cp[i].x {
        return cp[i].y;
    }

    // Handle points off of the right endpoint.
    let mut j = n - 1;
    if x >= cp[j].x {
        // skia-rust: libm (log2f)
        return cp[j].y + (cp[j].x / x).log2();
    }

    // Binary search for i, j bracket in which we find x.
    while j - i > 1 {
        let m = i.midpoint(j);
        if x < cp[m].x {
            j = m;
        } else {
            i = m;
        }
    }

    // Cache short names for the parameters for computing the cubic coefficients.
    let x_i = cp[i].x;
    let y_i = cp[i].y;
    let x_j = cp[j].x;
    let y_j = cp[j].y;
    let h_i = x_j - x_i;
    let m_hat_i = cp[i].m * h_i;
    let m_hat_j = cp[j].m * h_i;

    // Handle intervals that are a point.
    if h_i == 0.0 {
        return y_i;
    }

    // Compute the coefficients and evaluate the polynomial.
    let c3 = 2.0 * y_i + m_hat_i - 2.0 * y_j + m_hat_j;
    let c2 = -3.0 * y_i + 3.0 * y_j - 2.0 * m_hat_i - m_hat_j;
    let c1 = m_hat_i;
    let c0 = y_i;
    let t = (x - x_i) / h_i;

    ((c3 * t + c2) * t + c1) * t + c0
}

/// The function evaluation described in Clause 6.3.2 (`EvaluateColorGainFunction`).
// Port of: src/codec/SkHdrAgtm.cpp#L128-L140 (chrome/m156), EvaluateColorGainFunction
#[doc(alias = "EvaluateColorGainFunction")]
#[must_use]
pub fn evaluate_color_gain_function(gain: &ColorGainFunction, c: &Color4f) -> Color4f {
    let m = evaluate_component_mixing_function(&gain.component_mixing, c);
    let mut result = Color4f {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: c.a,
    };
    result.r = evaluate_gain_curve(&gain.gain_curve, m.r);
    if m.r == m.g && m.g == m.b {
        result.g = result.r;
        result.b = result.r;
    } else {
        result.g = evaluate_gain_curve(&gain.gain_curve, m.g);
        result.b = evaluate_gain_curve(&gain.gain_curve, m.b);
    }
    result
}

/// Populates the `m` values of a gain curve using the Piecewise Cubic Hermite Interpolation
/// Package (PCHIP) algorithm, described in Clause C.3.9 (`PopulateSlopeFromPCHIP`).
///
/// # Panics
///
/// Panics if the curve does not have between [`GainCurve::MIN_NUM_CONTROL_POINTS`] and
/// [`GainCurve::MAX_NUM_CONTROL_POINTS`] control points.
// Port of: src/codec/SkHdrAgtm.cpp#L142-L233 (chrome/m156), PopulateSlopeFromPCHIP
#[doc(alias = "PopulateSlopeFromPCHIP")]
pub fn populate_slope_from_pchip(gain_curve: &mut GainCurve) {
    let cp = &mut gain_curve.control_points;
    assert!(
        cp.len() >= GainCurve::MIN_NUM_CONTROL_POINTS
            && cp.len() <= GainCurve::MAX_NUM_CONTROL_POINTS
    );
    let n = cp.len();

    // Compute the interval width (h) and piecewise linear slope (s).
    let mut h = [0.0f32; GainCurve::MAX_NUM_CONTROL_POINTS];
    for i in 0..n - 1 {
        h[i] = cp[i + 1].x - cp[i].x;
    }
    let mut s = [0.0f32; GainCurve::MAX_NUM_CONTROL_POINTS];
    for i in 0..n - 1 {
        // Formula (C.7).
        s[i] = if h[i] != 0.0 {
            (cp[i + 1].y - cp[i].y) / h[i]
        } else {
            0.0
        };
    }

    // Common formulation for formulas (C.9) and (C.10).
    let three_point_finite_difference = |h0: f32, h1: f32, s0: f32, s1: f32| -> f32 {
        let num = (2.0 * h0 + h1) * s0 - h0 * s1;
        let den = h0 + h1;
        let mut m = num / den;
        if signbit(m) != signbit(s0) {
            m = 0.0;
        } else if signbit(m) != signbit(s1) && m.abs() > 3.0 * s0.abs() {
            m = 3.0 * s0;
        }
        m
    };

    for i in 0..n {
        let has_prev = i >= 1;
        if has_prev && i + 1 < n && cp[i - 1].x < cp[i].x && cp[i].x < cp[i + 1].x {
            // Interior point, formula (C.8).
            if signbit(s[i - 1]) != signbit(s[i]) || (s[i - 1] == 0.0 && s[i] == 0.0) {
                cp[i].m = 0.0;
            } else {
                let num = 3.0 * (h[i - 1] + h[i]) * s[i - 1] * s[i];
                let den = (2.0 * h[i - 1] + h[i]) * s[i - 1] + (h[i - 1] + 2.0 * h[i]) * s[i];
                cp[i].m = num / den;
            }
        } else if i + 1 < n && cp[i].x < cp[i + 1].x {
            // Left endpoint.
            if i + 2 < n && cp[i + 1].x < cp[i + 2].x {
                // Three-point finite difference, formula (C.9).
                cp[i].m = three_point_finite_difference(h[i], h[i + 1], s[i], s[i + 1]);
            } else {
                // Two-point finite difference.
                cp[i].m = s[i];
            }
        } else if has_prev && cp[i - 1].x < cp[i].x {
            // Right endpoint.
            if i >= 2 && cp[i - 2].x < cp[i - 1].x {
                // Three-point finite difference, formula (C.10).
                cp[i].m = three_point_finite_difference(h[i - 1], h[i - 2], s[i - 1], s[i - 2]);
            } else {
                // Two-point finite difference.
                cp[i].m = s[i - 1];
            }
        } else {
            cp[i].m = 0.0;
        }
    }
}

/// The weighting of the alternate images for a targeted HDR headroom, as computed by
/// `ComputeWeighting` (Clause 6.2.5).
// Port of: include/private/SkHdrMetadata.h#L389-L400 (chrome/m156), Weighting
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weighting {
    /// The index into `alternate_images` for `weight`. If `weight[i]` is 0 then
    /// `alternate_image_index[i]` is not used and is [`Weighting::K_INVALID_INDEX`].
    pub alternate_image_index: [u8; 2],
    /// The weight for the `alternate_image_index[i]`-th alternate image.
    pub weight: [f32; 2],
}

impl Weighting {
    /// The index used when no alternate image applies (`kInvalidIndex`).
    // Port of: include/private/SkHdrMetadata.h#L396 (chrome/m156), kInvalidIndex
    pub const K_INVALID_INDEX: u8 = 255;
}

impl Default for Weighting {
    // Port of: include/private/SkHdrMetadata.h#L397-L400 (chrome/m156), the in-class initializers
    fn default() -> Self {
        Self {
            alternate_image_index: [Self::K_INVALID_INDEX, Self::K_INVALID_INDEX],
            weight: [0.0, 0.0],
        }
    }
}

/// Computes the weighting for the specified targeted HDR headroom (`ComputeWeighting`), as in
/// Clause 6.2.5, Computation of the headroom-adaptive tone map.
// Port of: src/codec/SkHdrAgtm.cpp#L235-L306 (chrome/m156), ComputeWeighting
#[doc(alias = "ComputeWeighting")]
#[must_use]
pub fn compute_weighting(hatm: &HeadroomAdaptiveToneMap, targeted_hdr_headroom: f32) -> Weighting {
    let invalid = Weighting::K_INVALID_INDEX;
    let mut result = Weighting::default();

    // Create the list of HDR headrooms including the baseline image and all alternate images, as
    // described Clause 6.2.5 Computation of the headroom-adaptive tone map.

    // Let N be the length of the combined list.
    let mut n = 0usize;

    // Let H be the sorted list of HDR headrooms.
    let mut h = [0.0f32; HeadroomAdaptiveToneMap::MAX_NUM_ALTERNATE_IMAGES + 1];

    // Let indices list the index of each entry of H in alternate_images. The index for the
    // baseline HDR headroom is kInvalidIndex.
    let mut indices = [0u8; HeadroomAdaptiveToneMap::MAX_NUM_ALTERNATE_IMAGES + 1];
    let baseline = hatm.baseline_hdr_headroom;
    for (i, alt) in hatm.alternate_images.iter().enumerate() {
        if n == i && baseline < alt.hdr_headroom {
            // Insert the baseline HDR headroom before the indices as they are visited.
            indices[n] = invalid;
            h[n] = baseline;
            n += 1;
        }
        indices[n] = i as u8;
        h[n] = alt.hdr_headroom;
        n += 1;
    }
    if n == hatm.alternate_images.len() {
        // Insert the baseline HDR headroom at the end if it has not yet been inserted.
        indices[n] = invalid;
        h[n] = baseline;
        n += 1;
    }

    // Find the indices for the contributing images.
    if targeted_hdr_headroom <= h[0] {
        // One case of Formula (2), for the left endpoint.
        result.weight[0] = 1.0;
        result.alternate_image_index[0] = indices[0];
    } else if targeted_hdr_headroom >= h[n - 1] {
        // The other case of Formula (2), for the right endpoint.
        result.weight[0] = 1.0;
        result.alternate_image_index[0] = indices[n - 1];
    } else {
        // The case of Formula (3).
        let mut i = 0usize;
        while i < n - 1 {
            if h[i] <= targeted_hdr_headroom && targeted_hdr_headroom <= h[i + 1] {
                break;
            }
            i += 1;
        }
        result.weight[0] = (targeted_hdr_headroom - h[i + 1]) / (h[i] - h[i + 1]);
        result.weight[1] = 1.0 - result.weight[0];
        result.alternate_image_index[0] = indices[i];
        result.alternate_image_index[1] = indices[i + 1];
    }

    // The baseline image always has a gain of 0, so set the weight for the baseline image to 0.
    for i in 0..2 {
        if result.alternate_image_index[i] == invalid {
            result.weight[i] = 0.0;
        } else if result.weight[i] == 0.0 {
            result.alternate_image_index[i] = invalid;
        }
    }

    // Sort the weights so that the first weight is always greater.
    if result.weight[1] > result.weight[0] {
        result.weight.swap(0, 1);
        result.alternate_image_index.swap(0, 1);
    }

    result
}

/// Populates the metadata with the Reference White Tone Mapping Operator (RWTMO) parameters, based
/// on the value of the baseline HDR headroom (`PopulateUsingRwtmo`).
// Port of: src/codec/SkHdrAgtm.cpp#L308-L369 (chrome/m156), PopulateUsingRwtmo
#[doc(alias = "PopulateUsingRwtmo")]
pub fn populate_using_rwtmo(hatm: &mut HeadroomAdaptiveToneMap) {
    hatm.gain_application_space_primaries = named_primaries::REC2020;

    if hatm.baseline_hdr_headroom == 0.0 {
        hatm.alternate_images.clear();
        return;
    }

    // Set the two alternate image headrooms using Formula (C.1).
    // skia-rust: libm (log2f)
    hatm.alternate_images.resize(2, AlternateImage::default());
    hatm.alternate_images[0].hdr_headroom = 0.0;
    hatm.alternate_images[1].hdr_headroom = (8.0f32 / 3.0).log2()
        * std_min(hatm.baseline_hdr_headroom / (1000.0f32 / 203.0).log2(), 1.0);

    for a in 0..hatm.alternate_images.len() {
        let alt_headroom = hatm.alternate_images[a].hdr_headroom;
        let gain: &mut ColorGainFunction = &mut hatm.alternate_images[a].color_gain_function;
        *gain = ColorGainFunction::default();

        // Use maxRGB for applying the curve.
        gain.component_mixing.max = 1.0;

        // Compute the image of white under the tone mapping from Formula (C.2).
        let y_white = if a == 1 {
            1.0
        } else {
            1.0 - 0.5 * std_min(hatm.baseline_hdr_headroom / (1000.0f32 / 203.0).log2(), 1.0)
        };

        // Compute the Bezier control points using Formula (C.3).
        let kappa = 0.65f32;
        let x_knee = 1.0f32;
        let y_knee = y_white;
        let x_max = hatm.baseline_hdr_headroom.exp2();
        let y_max = alt_headroom.exp2();
        let x_mid = (1.0 - kappa) * x_knee + kappa * (x_knee * y_max / y_knee);
        let y_mid = (1.0 - kappa) * y_knee + kappa * y_max;

        // Compute the cubic coefficients using Formula (C.5).
        let x_a = x_knee - 2.0 * x_mid + x_max;
        let y_a = y_knee - 2.0 * y_mid + y_max;
        let x_b = 2.0 * x_mid - 2.0 * x_knee;
        let y_b = 2.0 * y_mid - 2.0 * y_knee;
        let x_c = x_knee;
        let y_c = y_knee;

        let num_control_points = 8usize;
        let cubic: &mut GainCurve = &mut gain.gain_curve;
        cubic.control_points = vec![GainCurveControlPoint::default(); num_control_points];
        for (c, cp) in cubic.control_points.iter_mut().enumerate() {
            // Compute the linear domain curve values using Formula (C.4).
            let t = c as f32 / (num_control_points as f32 - 1.0);
            let x = x_c + t * (x_b + t * x_a);
            let y = y_c + t * (y_b + t * y_a);
            let m = (2.0 * y_a * t + y_b) / (2.0 * x_a * t + x_b);

            // Compute the log domain curve values using Formula (C.6).
            cp.x = x;
            cp.y = (y / x).log2();
            cp.m = (x * m - y) / (2.0f32.ln() * x * y);
        }
    }
}

/// Applies the tone mapping to `colors` in the gain application color space, targeting the
/// specified `targeted_hdr_headroom` (`ApplyGain`).
// Port of: src/codec/SkHdrAgtm.cpp#L371-L414 (chrome/m156), ApplyGain
#[doc(alias = "ApplyGain")]
pub fn apply_gain(
    hatm: &HeadroomAdaptiveToneMap,
    colors: &mut [Color4f],
    targeted_hdr_headroom: f32,
) {
    debug_assert!(validate_hatm(hatm));

    // This function implements Formula (4). It creates special cases for one or both of the
    // weights being zero.
    let weighting = compute_weighting(hatm, targeted_hdr_headroom);

    if weighting.weight[0] == 0.0 {
        // If no weight is non-zero, then no gain will be applied. Leave the points unchanged.
    } else if weighting.weight[1] == 0.0 {
        // Special case the case of there being only one weighted gain function.
        let gain = &hatm.alternate_images[usize::from(weighting.alternate_image_index[0])]
            .color_gain_function;
        let w = weighting.weight[0];
        for c in colors.iter_mut() {
            let g = evaluate_color_gain_function(gain, c);
            *c = Color4f {
                r: c.r * (w * g.r).exp2(),
                g: c.g * (w * g.g).exp2(),
                b: c.b * (w * g.b).exp2(),
                a: c.a,
            };
        }
    } else {
        // The general case of two weighted gain functions.
        let gain0 = &hatm.alternate_images[usize::from(weighting.alternate_image_index[0])]
            .color_gain_function;
        let w0 = weighting.weight[0];
        let gain1 = &hatm.alternate_images[usize::from(weighting.alternate_image_index[1])]
            .color_gain_function;
        let w1 = weighting.weight[1];
        for c in colors.iter_mut() {
            let g0 = evaluate_color_gain_function(gain0, c);
            let g1 = evaluate_color_gain_function(gain1, c);
            *c = Color4f {
                r: c.r * (w0 * g0.r + w1 * g1.r).exp2(),
                g: c.g * (w0 * g0.g + w1 * g1.g).exp2(),
                b: c.b * (w0 * g0.b + w1 * g1.b).exp2(),
                a: c.a,
            };
        }
    }
}

/// Returns the gain application color space: linear transfer with the gain application
/// primaries (`GetGainApplicationSpace`).
// Port of: src/codec/SkHdrAgtm.cpp#L416-L423 (chrome/m156), GetGainApplicationSpace
#[doc(alias = "GetGainApplicationSpace")]
#[must_use]
pub fn get_gain_application_space(hatm: &HeadroomAdaptiveToneMap) -> Option<ColorSpace> {
    let to_xyz_d50 = hatm.gain_application_space_primaries.to_xyzd50()?;
    ColorSpace::new_rgb(&named_transfer_fn::LINEAR, &to_xyz_d50)
}

/// The maximum luminance from CLLI, MDCV, or a default of 1000 nits.
// Port of: src/codec/SkHdrAgtm.cpp#L427-L442 (chrome/m156), get_max_luminance
fn get_max_luminance(metadata: &Metadata) -> f32 {
    if let Some(clli) = metadata.get_content_light_level_information()
        && clli.max_cll > 0.0
    {
        return clli.max_cll;
    }
    if let Some(mdcv) = metadata.get_mastering_display_color_volume()
        && mdcv.maximum_display_mastering_luminance > 0.0
    {
        return mdcv.maximum_display_mastering_luminance;
    }
    1000.0
}

/// If tone mapping is to be performed for `input_color_space` with `metadata`, returns the AGTM
/// parameters to use with the color filter and the scale factor to apply first. If tone mapping
/// should not be performed, returns `None` (`PopulateToneMapAgtmParams`).
// Port of: src/codec/SkHdrAgtm.cpp#L444-L496 (chrome/m156), PopulateToneMapAgtmParams
#[doc(alias = "PopulateToneMapAgtmParams")]
#[must_use]
pub fn populate_tone_map_agtm_params(
    metadata: &Metadata,
    input_color_space: Option<&ColorSpace>,
) -> Option<(AdaptiveGlobalToneMap, f32)> {
    // If `input_color_space` is HLG or PQ, find the HDR reference white value. When the shader
    // starts, this is the luminance that will have been mapped to 1.0. The scale factor is such
    // that the AGTM HDR reference white luminance (if specified) will be mapped to 1.0.
    let mut input_is_pq_or_hlg = false;
    let mut input_pq_or_hlg_white = AdaptiveGlobalToneMap::DEFAULT_HDR_REFERENCE_WHITE;
    if let Some(cs) = input_color_space {
        let trfn = cs.transfer_fn();
        match trfn.tf_type() {
            skia_rust_skcms::TfType::PQ | skia_rust_skcms::TfType::HLG => {
                input_is_pq_or_hlg = true;
                input_pq_or_hlg_white = trfn.a;
            }
            _ => {}
        }
    }

    let existing = metadata.get_adaptive_global_tone_map();
    let had_agtm_metadata = existing.is_some();
    let mut agtm = existing.unwrap_or_default();

    // SDR content that does not specify an inverse tone mapping will not have a default tone
    // mapping added.
    if !input_is_pq_or_hlg && (!had_agtm_metadata || agtm.headroom_adaptive_tone_map.is_none()) {
        return None;
    }

    // If no AGTM was specified, populate the HDR reference white from the input color space.
    if !had_agtm_metadata {
        agtm.hdr_reference_white = input_pq_or_hlg_white;
    }

    // If no tone mapping was specified, then use RWTMO with the baseline HDR headroom computed
    // from the CLLI and MDCV metadata.
    if agtm.headroom_adaptive_tone_map.is_none() {
        // skia-rust: libm (log2f)
        let baseline = std_max(get_max_luminance(metadata) / agtm.hdr_reference_white, 1.0).log2();
        let mut hatm = HeadroomAdaptiveToneMap {
            baseline_hdr_headroom: baseline,
            ..Default::default()
        };
        populate_using_rwtmo(&mut hatm);
        agtm.headroom_adaptive_tone_map = Some(hatm);
    }

    let scale_factor = if input_is_pq_or_hlg {
        input_pq_or_hlg_white / agtm.hdr_reference_white
    } else {
        1.0
    };
    Some((agtm, scale_factor))
}

/// Return true if `agtm` satisfies all normative constraints (`Validate`).
// Port of: src/codec/SkHdrAgtm.cpp#L498-L511 (chrome/m156), Validate
#[doc(alias = "Validate")]
#[must_use]
pub fn validate(agtm: &AdaptiveGlobalToneMap) -> bool {
    if agtm.hdr_reference_white < 0.0 || agtm.hdr_reference_white > 10000.0 {
        return false;
    }
    if let Some(hatm) = &agtm.headroom_adaptive_tone_map
        && !validate_hatm(hatm)
    {
        return false;
    }
    true
}

/// Return true if `hatm` satisfies all normative constraints (`Validate`).
// Port of: src/codec/SkHdrAgtm.cpp#L513-L656 (chrome/m156), Validate (HeadroomAdaptiveToneMap)
#[doc(alias = "Validate")]
#[must_use]
pub fn validate_hatm(hatm: &HeadroomAdaptiveToneMap) -> bool {
    // The maximum and minimum HDR headroom values allowed by the specification.
    const MIN_HDR_HEADROOM: f32 = 0.0;
    const MAX_HDR_HEADROOM: f32 = 6.0;
    // The maximum linear value is exp2(MAX_HDR_HEADROOM) = 64.
    const MAX_LINEAR_HDR_HEADROOM: f32 = 64.0;

    let in_unit_interval = |x: f32| (0.0..=1.0).contains(&x);

    // Enforce the constraints listed in Clause 6.2.2.
    {
        // The value of H_baseline shall be in the interval [0, 6].
        if hatm.baseline_hdr_headroom < MIN_HDR_HEADROOM
            || hatm.baseline_hdr_headroom > MAX_HDR_HEADROOM
        {
            return false;
        }
        // The value of N_alt shall be in the set {0,1,2,3,4}.
        if hatm.alternate_images.len() > HeadroomAdaptiveToneMap::MAX_NUM_ALTERNATE_IMAGES {
            return false;
        }
        // H_alt,i shall not equal H_baseline.
        for alt in &hatm.alternate_images {
            if alt.hdr_headroom == hatm.baseline_hdr_headroom {
                return false;
            }
        }
        // H_{alt,i} shall be strictly less than H_{alt,i+1}
        for a in 1..hatm.alternate_images.len() {
            if hatm.alternate_images[a].hdr_headroom <= hatm.alternate_images[a - 1].hdr_headroom {
                return false;
            }
        }
        // Chromaticity coordinates must be in the unit interval.
        let p = &hatm.gain_application_space_primaries;
        if !in_unit_interval(p.rx)
            || !in_unit_interval(p.ry)
            || !in_unit_interval(p.gx)
            || !in_unit_interval(p.gy)
            || !in_unit_interval(p.bx)
            || !in_unit_interval(p.by)
            || !in_unit_interval(p.wx)
            || !in_unit_interval(p.wy)
        {
            return false;
        }
    }

    // Enforce the constraints listed in Clause 6.2.3 that are required for the metadata to be
    // encodable. The specification indicates that "implementations may adjust metadata items so
    // that they satisfy these constraints" for the other constraints.
    for alt in &hatm.alternate_images {
        for cp in &alt.color_gain_function.gain_curve.control_points {
            if alt.hdr_headroom > hatm.baseline_hdr_headroom {
                // If H_{alt,i} > H_baseline then GainCurve_i(x) >= 0.
                if cp.y < 0.0 {
                    return false;
                }
            } else {
                // If H_{alt,i} < H_baseline then GainCurve_i(x) <= 0.
                if cp.y > 0.0 {
                    return false;
                }
            }
        }
    }

    // Enforce the constraints listed in Clause 6.4.2.
    for alt in &hatm.alternate_images {
        let mix = &alt.color_gain_function.component_mixing;
        // All components shall be in the interval [0, 1].
        if !in_unit_interval(mix.red)
            || !in_unit_interval(mix.green)
            || !in_unit_interval(mix.blue)
            || !in_unit_interval(mix.max)
            || !in_unit_interval(mix.min)
            || !in_unit_interval(mix.component)
        {
            return false;
        }
        // The sum of all components shall equal 1. The coefficients are encoded in steps of
        // 0.00001. Allow the sum to be at most one step away from 1.
        let sum = mix.red + mix.green + mix.blue + mix.max + mix.min + mix.component;
        if sum < 0.99999 || sum > 1.00001 {
            return false;
        }
    }

    // Enforce the constraints listed in Clause 6.5.2.
    for alt in &hatm.alternate_images {
        let curve = &alt.color_gain_function.gain_curve;
        // The value of N_cp must be in the set {1,...,32}.
        if curve.control_points.len() < GainCurve::MIN_NUM_CONTROL_POINTS
            || curve.control_points.len() > GainCurve::MAX_NUM_CONTROL_POINTS
        {
            return false;
        }
        for (c, cp) in curve.control_points.iter().enumerate() {
            // The value of x_i shall be in the interval [0, 64].
            if cp.x < 0.0 || cp.x > MAX_LINEAR_HDR_HEADROOM {
                return false;
            }
            // The value of y_i shall be in the interval [-6, 6].
            if cp.y < -MAX_HDR_HEADROOM || cp.y > MAX_HDR_HEADROOM {
                return false;
            }
            if c > 0 {
                let prev = &curve.control_points[c - 1];
                // It shall be the case that x_i <= x_{i+1}.
                if cp.x < prev.x {
                    return false;
                }
                // If x_i == x_{i+1} then it shall be the case that y_i == y_{i+1}.
                if cp.x == prev.x && cp.y != prev.y {
                    return false;
                }
            }
        }
    }
    true
}
