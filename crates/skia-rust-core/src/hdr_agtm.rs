// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/private/SkHdrMetadata.h#L146-L380 and src/codec/SkHdrAgtmParse.cpp and
// src/codec/SkHdrMetadata.cpp#L165-L259 (chrome/m156): the adaptive global tone map (AGTM) metadata
// and its binary encoding (ANSI/SMPTE ST 2094-50 Annex C), and the `skhdr::Metadata` container.
// The tone mapping helpers are in `hdr_agtm_helpers`.

//! Adaptive global tone map (AGTM) metadata and the HDR metadata container.

// The ported code keeps the C++ arithmetic: float equality is exact by design (the Annex C
// encodings compare exactly), the integer widths of the syntax elements are converted as the C++
// converts them (`std::lround`, `uint16_t` clamping), the bitfields are individual flags, and the
// parser and serializer keep the structure of the Skia source, so they are long.
#![allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::nonminimal_bool,
    clippy::if_not_else
)]

use crate::color_space::{ColorSpacePrimaries, named_primaries};
use crate::data::Data;
use crate::hdr_metadata::{ContentLightLevelInformation, MasteringDisplayColorVolume};

/// A control point of an AGTM gain curve (`AdaptiveGlobalToneMap::GainCurve::ControlPoint`).
// Port of: include/private/SkHdrMetadata.h#L160-L177 (chrome/m156), GainCurve::ControlPoint
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GainCurveControlPoint {
    /// The x coordinate (`fX`).
    pub x: f32,
    /// The y coordinate (`fY`).
    pub y: f32,
    /// The slope at this point (`fM`).
    pub m: f32,
}

/// A gain curve: a list of control points (`AdaptiveGlobalToneMap::GainCurve`).
// Port of: include/private/SkHdrMetadata.h#L160-L177 (chrome/m156), GainCurve
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GainCurve {
    /// The control points (`fControlPoints`).
    pub control_points: Vec<GainCurveControlPoint>,
}

impl GainCurve {
    /// The fewest control points a gain curve may have (`kMinNumControlPoints`).
    // Port of: include/private/SkHdrMetadata.h#L183 (chrome/m156), kMinNumControlPoints
    pub const MIN_NUM_CONTROL_POINTS: usize = 1;
    /// The most control points a gain curve may have (`kMaxNumControlPoints`).
    // Port of: include/private/SkHdrMetadata.h#L184 (chrome/m156), kMaxNumControlPoints
    pub const MAX_NUM_CONTROL_POINTS: usize = 32;
}

/// The component mixing function (`AdaptiveGlobalToneMap::ComponentMixingFunction`).
// Port of: include/private/SkHdrMetadata.h#L190-L203 (chrome/m156), ComponentMixingFunction
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ComponentMixingFunction {
    /// `fRed`.
    pub red: f32,
    /// `fGreen`.
    pub green: f32,
    /// `fBlue`.
    pub blue: f32,
    /// `fMax`.
    pub max: f32,
    /// `fMin`.
    pub min: f32,
    /// `fComponent`.
    pub component: f32,
}

/// The colour gain function of an alternate image (`AdaptiveGlobalToneMap::ColorGainFunction`).
// Port of: include/private/SkHdrMetadata.h#L205-L213 (chrome/m156), ColorGainFunction
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorGainFunction {
    /// The component mixing group (`fComponentMixing`).
    pub component_mixing: ComponentMixingFunction,
    /// The gain curve group (`fGainCurve`).
    pub gain_curve: GainCurve,
}

/// An alternate image (`AdaptiveGlobalToneMap::AlternateImage`).
// Port of: include/private/SkHdrMetadata.h#L215-L224 (chrome/m156), AlternateImage
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AlternateImage {
    /// The HDR headroom of the alternate image (`fHdrHeadroom`).
    pub hdr_headroom: f32,
    /// The colour gain function (`fColorGainFunction`).
    pub color_gain_function: ColorGainFunction,
}

/// The headroom adaptive tone map (`AdaptiveGlobalToneMap::HeadroomAdaptiveToneMap`).
// Port of: include/private/SkHdrMetadata.h#L226-L244 (chrome/m156), HeadroomAdaptiveToneMap
#[derive(Clone, Debug, PartialEq)]
pub struct HeadroomAdaptiveToneMap {
    /// The baseline HDR headroom (`fBaselineHdrHeadroom`).
    pub baseline_hdr_headroom: f32,
    /// The gain application space primaries (`fGainApplicationSpacePrimaries`).
    pub gain_application_space_primaries: ColorSpacePrimaries,
    /// The alternate images (`fAlternateImages`).
    pub alternate_images: Vec<AlternateImage>,
}

impl HeadroomAdaptiveToneMap {
    /// The most alternate images allowed (`kMaxNumAlternateImages`).
    // Port of: include/private/SkHdrMetadata.h#L237 (chrome/m156), kMaxNumAlternateImages
    pub const MAX_NUM_ALTERNATE_IMAGES: usize = 4;
}

impl Default for HeadroomAdaptiveToneMap {
    // Port of: include/private/SkHdrMetadata.h#L229-L234 (chrome/m156), the in-class initializers
    fn default() -> Self {
        Self {
            baseline_hdr_headroom: 0.0,
            gain_application_space_primaries: ColorSpacePrimaries::new(
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ),
            alternate_images: Vec::new(),
        }
    }
}

/// Adaptive Global Tone Map metadata (`skhdr::AdaptiveGlobalToneMap`), from SMPTE ST 2094-50.
// Port of: include/private/SkHdrMetadata.h#L146-L247 (chrome/m156), AdaptiveGlobalToneMap
#[doc(alias = "skhdr::AdaptiveGlobalToneMap")]
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveGlobalToneMap {
    /// The HDR reference white, in nits (`fHdrReferenceWhite`).
    pub hdr_reference_white: f32,
    /// The headroom adaptive tone map group, if present (`fHeadroomAdaptiveToneMap`).
    pub headroom_adaptive_tone_map: Option<HeadroomAdaptiveToneMap>,
}

impl Default for AdaptiveGlobalToneMap {
    // Port of: include/private/SkHdrMetadata.h#L249-L255 (chrome/m156), the in-class initializers
    fn default() -> Self {
        Self {
            hdr_reference_white: Self::DEFAULT_HDR_REFERENCE_WHITE,
            headroom_adaptive_tone_map: None,
        }
    }
}

impl AdaptiveGlobalToneMap {
    /// The default HDR reference white, in nits (`kDefaultHdrReferenceWhite`).
    // Port of: include/private/SkHdrMetadata.h#L249 (chrome/m156), kDefaultHdrReferenceWhite
    pub const DEFAULT_HDR_REFERENCE_WHITE: f32 = 203.0;

    /// Decodes the binary encoding of Annex C. Returns `None` if parsing fails.
    // Port of: src/codec/SkHdrAgtmParse.cpp#L423-L520 (chrome/m156), AdaptiveGlobalToneMap::parse
    #[must_use]
    pub fn parse(data: &Data) -> Option<Self> {
        let mut s = ByteReader::new(data.as_bytes());
        let mut syntax = AgtmSyntax::default();
        syntax.parse_application_info(&mut s)?;

        // Semantics from Clause C.3.3.
        let hdr_reference_white = if syntax.has_custom_hdr_reference_white_flag {
            uint16_to_float(syntax.hdr_reference_white, 1, 50000, 0, 5.0)
        } else {
            Self::DEFAULT_HDR_REFERENCE_WHITE
        };
        let mut agtm = Self {
            hdr_reference_white,
            headroom_adaptive_tone_map: None,
        };
        if !syntax.has_adaptive_tone_map_flag {
            return Some(agtm);
        }
        // Semantics from Clause C.3.4.
        let mut hatm = HeadroomAdaptiveToneMap {
            baseline_hdr_headroom: uint16_to_float(
                syntax.baseline_hdr_headroom,
                0,
                60000,
                0,
                10000.0,
            ),
            ..Default::default()
        };
        if syntax.use_reference_white_tone_mapping_flag {
            crate::hdr_agtm_helpers::populate_using_rwtmo(&mut hatm);
            agtm.headroom_adaptive_tone_map = Some(hatm);
            return Some(agtm);
        }

        let num_alternate_images = usize::from(syntax.num_alternate_images)
            .min(HeadroomAdaptiveToneMap::MAX_NUM_ALTERNATE_IMAGES);
        hatm.alternate_images = vec![AlternateImage::default(); num_alternate_images];
        for (a, alt) in hatm.alternate_images.iter_mut().enumerate() {
            alt.hdr_headroom =
                uint16_to_float(syntax.alternate_hdr_headrooms[a], 0, 60000, 0, 10000.0);
        }

        // Semantics from Clause C.3.5.
        hatm.gain_application_space_primaries =
            match syntax.gain_application_space_chromaticities_flag {
                0 => named_primaries::REC709,
                1 => named_primaries::SMPTE_EG_432_1,
                2 => named_primaries::REC2020,
                _ => {
                    let c = &syntax.gain_application_space_chromaticities;
                    let f = |i: usize| uint16_to_float(c[i], 0, 50000, 0, 50000.0);
                    ColorSpacePrimaries {
                        rx: f(0),
                        ry: f(1),
                        gx: f(2),
                        gy: f(3),
                        bx: f(4),
                        by: f(5),
                        wx: f(6),
                        wy: f(7),
                    }
                }
            };

        // Semantics from Clause C.3.6.
        for (a, alt) in hatm.alternate_images.iter_mut().enumerate() {
            let mix = &mut alt.color_gain_function.component_mixing;
            match syntax.component_mixing_type[a] {
                0 => {
                    *mix = ComponentMixingFunction {
                        max: 1.0,
                        ..Default::default()
                    };
                }
                1 => {
                    *mix = ComponentMixingFunction {
                        component: 1.0,
                        ..Default::default()
                    };
                }
                2 => {
                    *mix = ComponentMixingFunction {
                        red: 1.0 / 6.0,
                        green: 1.0 / 6.0,
                        blue: 1.0 / 6.0,
                        max: 1.0 / 2.0,
                        ..Default::default()
                    };
                }
                _ => {
                    let mut p = [0.0f32; AgtmSyntax::NUM_MIX_COEFFICIENTS];
                    let mut p_sum = 0.0f32;
                    for (k, pk) in p.iter_mut().enumerate() {
                        *pk = uint16_to_float(
                            syntax.component_mixing_coefficient[a][k],
                            0,
                            50000,
                            0,
                            50000.0,
                        );
                        p_sum += *pk;
                    }
                    if p_sum == 0.0 {
                        return None;
                    }
                    *mix = ComponentMixingFunction {
                        red: p[0] / p_sum,
                        green: p[1] / p_sum,
                        blue: p[2] / p_sum,
                        max: p[3] / p_sum,
                        min: p[4] / p_sum,
                        component: p[5] / p_sum,
                    };
                }
            }
        }

        // Semantics from Clause C.3.7.
        for (a, alt) in hatm.alternate_images.iter_mut().enumerate() {
            let num_control_points =
                usize::from(syntax.gain_curve_num_control_points_minus_1[a]) + 1;
            let cubic = &mut alt.color_gain_function.gain_curve;
            cubic.control_points = vec![GainCurveControlPoint::default(); num_control_points];
            let sign = if syntax.baseline_hdr_headroom < syntax.alternate_hdr_headrooms[a] {
                1.0f32
            } else {
                -1.0f32
            };
            for (c, cp) in cubic.control_points.iter_mut().enumerate() {
                cp.x = uint16_to_float(
                    syntax.gain_curve_control_points_x[a][c],
                    0,
                    64000,
                    0,
                    1000.0,
                );
                cp.y = sign
                    * uint16_to_float(
                        syntax.gain_curve_control_points_y[a][c],
                        0,
                        60000,
                        0,
                        10000.0,
                    );
            }
            if syntax.gain_curve_use_pchip_slope_flag[a] {
                crate::hdr_agtm_helpers::populate_slope_from_pchip(cubic);
            } else {
                for (c, cp) in cubic.control_points.iter_mut().enumerate() {
                    let theta = uint16_to_float(
                        syntax.gain_curve_control_points_theta[a][c],
                        1,
                        35999,
                        18000,
                        36000.0 / std::f32::consts::PI,
                    );
                    // skia-rust: libm (tanf)
                    cp.m = theta.tan();
                }
            }
        }

        agtm.headroom_adaptive_tone_map = Some(hatm);

        // Reject any metadata that violates the normative constraints.
        if !agtm.is_valid() {
            return None;
        }
        Some(agtm)
    }

    /// Serializes to the encoding that [`parse`](Self::parse) reads. Returns `None` if this
    /// metadata is not valid.
    // Port of: src/codec/SkHdrAgtmParse.cpp#L522-L664 (chrome/m156), AdaptiveGlobalToneMap::serialize
    #[must_use]
    pub fn serialize(&self) -> Option<Data> {
        // Reject requests to serialize invalid metadata.
        if !self.is_valid() {
            return None;
        }

        // Populate `syntax` according to the semantics in Clause C.3.
        let mut syntax = AgtmSyntax {
            application_version: 0,
            minimum_application_version: 0,
            has_custom_hdr_reference_white_flag: self.hdr_reference_white
                != Self::DEFAULT_HDR_REFERENCE_WHITE,
            ..Default::default()
        };
        if syntax.has_custom_hdr_reference_white_flag {
            syntax.hdr_reference_white =
                float_to_uint16(self.hdr_reference_white, 1, 50000, 0, 5.0);
        }

        if let Some(hatm) = &self.headroom_adaptive_tone_map {
            syntax.has_adaptive_tone_map_flag = true;

            // Semantics from Clause C.3.4.
            syntax.baseline_hdr_headroom =
                float_to_uint16(hatm.baseline_hdr_headroom, 0, 60000, 0, 10000.0);

            // If the headroom-adaptive tone map is equal to RWTMO, then mark it as such.
            let mut hatm_rwtmo = hatm.clone();
            crate::hdr_agtm_helpers::populate_using_rwtmo(&mut hatm_rwtmo);
            syntax.use_reference_white_tone_mapping_flag = *hatm == hatm_rwtmo;

            syntax.num_alternate_images = hatm.alternate_images.len() as u8;
            for (a, alt) in hatm.alternate_images.iter().enumerate() {
                syntax.alternate_hdr_headrooms[a] =
                    float_to_uint16(alt.hdr_headroom, 0, 60000, 0, 10000.0);
            }

            // Semantics from Clause C.3.5.
            let primaries = &hatm.gain_application_space_primaries;
            syntax.gain_application_space_chromaticities_flag =
                if *primaries == named_primaries::REC709 {
                    0
                } else if *primaries == named_primaries::SMPTE_EG_432_1 {
                    1
                } else if *primaries == named_primaries::REC2020 {
                    2
                } else {
                    3
                };
            if syntax.gain_application_space_chromaticities_flag == 3 {
                let values = [
                    primaries.rx,
                    primaries.ry,
                    primaries.gx,
                    primaries.gy,
                    primaries.bx,
                    primaries.by,
                    primaries.wx,
                    primaries.wy,
                ];
                for (i, v) in values.into_iter().enumerate() {
                    syntax.gain_application_space_chromaticities[i] =
                        float_to_uint16(v, 0, 50000, 0, 50000.0);
                }
            }

            // Semantics from Clause C.3.6.
            syntax.has_common_component_mix_params_flag = false;
            for (a, alt) in hatm.alternate_images.iter().enumerate() {
                let mix = &alt.color_gain_function.component_mixing;
                if mix.red == 0.0
                    && mix.green == 0.0
                    && mix.blue == 0.0
                    && mix.max == 1.0
                    && mix.min == 0.0
                    && mix.component == 0.0
                {
                    syntax.component_mixing_type[a] = 0;
                } else if mix.red == 0.0
                    && mix.green == 0.0
                    && mix.blue == 0.0
                    && mix.max == 0.0
                    && mix.min == 0.0
                    && mix.component == 1.0
                {
                    syntax.component_mixing_type[a] = 1;
                } else if mix.red == 1.0 / 6.0
                    && mix.green == 1.0 / 6.0
                    && mix.blue == 1.0 / 6.0
                    && mix.max == 1.0 / 2.0
                    && mix.min == 0.0
                    && mix.component == 0.0
                {
                    syntax.component_mixing_type[a] = 2;
                } else {
                    syntax.component_mixing_type[a] = 3;
                    let values = [
                        mix.red,
                        mix.green,
                        mix.blue,
                        mix.max,
                        mix.min,
                        mix.component,
                    ];
                    for (k, v) in values.into_iter().enumerate() {
                        syntax.component_mixing_coefficient[a][k] =
                            float_to_uint16(v, 0, 50000, 0, 50000.0);
                    }
                    for k in 0..AgtmSyntax::NUM_MIX_COEFFICIENTS {
                        syntax.has_component_mixing_coefficient_flag[a][k] =
                            syntax.component_mixing_coefficient[a][k] != 0;
                    }
                }
            }

            // Semantics from Clause C.3.7.
            syntax.has_common_curve_params_flag = false;
            for (a, alt) in hatm.alternate_images.iter().enumerate() {
                let sign = if hatm.baseline_hdr_headroom < alt.hdr_headroom {
                    1.0f32
                } else {
                    -1.0f32
                };

                let cubic = &alt.color_gain_function.gain_curve;
                syntax.gain_curve_num_control_points_minus_1[a] =
                    (cubic.control_points.len() - 1) as u8;
                // TODO(https://crbug.com/395659818): Identify when slope is equal to PCHIP to
                // further compress the serialization.
                syntax.gain_curve_use_pchip_slope_flag[a] = false;
                for (c, cp) in cubic.control_points.iter().enumerate() {
                    syntax.gain_curve_control_points_x[a][c] =
                        float_to_uint16(cp.x, 0, 64000, 0, 1000.0);
                }
                for (c, cp) in cubic.control_points.iter().enumerate() {
                    syntax.gain_curve_control_points_y[a][c] =
                        float_to_uint16(sign * cp.y, 0, 60000, 0, 10000.0);
                }
                for (c, cp) in cubic.control_points.iter().enumerate() {
                    // skia-rust: libm (atanf)
                    let theta = cp.m.atan();
                    syntax.gain_curve_control_points_theta[a][c] =
                        float_to_uint16(theta, 1, 35999, 18000, 36000.0 / std::f32::consts::PI);
                }
            }
        }

        // Write the syntax according to Clause C.2.
        let mut out = Vec::new();
        syntax.write_application_info(&mut out);
        Some(Data::new_from_vec(out))
    }

    /// Return true if this metadata satisfies all normative constraints.
    // Port of: src/codec/SkHdrAgtm.cpp#L837-L839 (chrome/m156), AdaptiveGlobalToneMap::isValid
    #[must_use]
    pub fn is_valid(&self) -> bool {
        crate::hdr_agtm_helpers::validate(self)
    }
}

/// Port of `clamp`: `min(max(x, a), b)`.
// Port of: src/codec/SkHdrAgtmParse.cpp#L17-L21 (chrome/m156), clamp
fn clamp_i32(x: i32, a: i32, b: i32) -> i32 {
    x.max(a).min(b)
}

/// Converts `f` by scaling by `scale`, offsetting by `offset`, and then clamping the result to
/// [`clamp_min`, `clamp_max`].
// Port of: src/codec/SkHdrAgtmParse.cpp#L23-L30 (chrome/m156), float_to_uint16
fn float_to_uint16(f: f32, clamp_min: u16, clamp_max: u16, offset: u16, scale: f32) -> u16 {
    // `std::lround` rounds halfway cases away from zero, as `f32::round` does.
    let rounded = (f * scale).round() as i64 as i32;
    let v = rounded.wrapping_add(i32::from(offset));
    clamp_i32(v, i32::from(clamp_min), i32::from(clamp_max)) as u16
}

/// The inverse of [`float_to_uint16`]'s conversion.
// Port of: src/codec/SkHdrAgtmParse.cpp#L32-L36 (chrome/m156), uint16_to_float
fn uint16_to_float(v: u16, clamp_min: u16, clamp_max: u16, offset: u16, scale: f32) -> f32 {
    let clamped = clamp_i32(i32::from(v), i32::from(clamp_min), i32::from(clamp_max));
    (clamped - i32::from(offset)) as f32 / scale
}

/// A cursor over the bytes of an encoded metadata, as `SkMemoryStream` reads them.
struct ByteReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn read_u8(&mut self) -> Option<u8> {
        let v = *self.bytes.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }

    // Port of: include/core/SkStream.h (chrome/m156), SkStreamPriv::ReadU16BE
    fn read_u16_be(&mut self) -> Option<u16> {
        let hi = self.read_u8()?;
        let lo = self.read_u8()?;
        Some(u16::from_be_bytes([hi, lo]))
    }
}

/// Reads the individual fields of one bitfield byte, most significant bits first.
// Port of: src/codec/SkHdrAgtmParse.cpp#L44-L67 (chrome/m156), BitfieldReader
struct BitfieldReader {
    bits: u8,
}

impl BitfieldReader {
    fn read_from(s: &mut ByteReader<'_>) -> Option<Self> {
        Some(Self { bits: s.read_u8()? })
    }

    fn read_bits(&mut self, bits: u32) -> u8 {
        // Read the topmost `bits` bits, then shift them off.
        let result = self.bits >> (8 - bits);
        self.bits = self.bits.checked_shl(bits).unwrap_or(0);
        result
    }

    fn read_flag(&mut self) -> bool {
        self.read_bits(1) == 1
    }
}

/// Writes the individual fields of one bitfield byte, then pads it with zeros.
// Port of: src/codec/SkHdrAgtmParse.cpp#L69-L93 (chrome/m156), BitfieldWriter
#[derive(Default)]
struct BitfieldWriter {
    bits: u8,
    bits_written: u32,
}

impl BitfieldWriter {
    fn write_bits(&mut self, value: u8, bits: u32) {
        self.bits = self.bits.checked_shl(bits).unwrap_or(0);
        self.bits |= value;
        self.bits_written += bits;
    }

    fn write_flag(&mut self, value: bool) {
        self.write_bits(u8::from(value), 1);
    }

    fn pad_and_write_to(self, out: &mut Vec<u8>) {
        // Write the remaining bits as zero, then write the result.
        let bits = self.bits.checked_shl(8 - self.bits_written).unwrap_or(0);
        out.push(bits);
    }
}

/// All syntax elements of Annex C.2 (excluding `reserved_zero`). Parsing and serializing is done
/// in two steps: converting to and from the metadata items (Annex C.3), and the syntax elements to
/// and from bytes (Annex C.2).
// Port of: src/codec/SkHdrAgtmParse.cpp#L95-L175 (chrome/m156), AgtmSyntax
#[derive(Default)]
struct AgtmSyntax {
    // Syntax elements of smpte_st_2094_50_application_info().
    application_version: u8,
    minimum_application_version: u8,

    // Syntax elements of smpte_st_2094_50_color_volume_transform().
    has_custom_hdr_reference_white_flag: bool,
    has_adaptive_tone_map_flag: bool,
    hdr_reference_white: u16,

    // Syntax elements of smpte_st_2094_50_adaptive_tone_map().
    baseline_hdr_headroom: u16,
    use_reference_white_tone_mapping_flag: bool,
    num_alternate_images: u8,
    gain_application_space_chromaticities_flag: u8,
    has_common_component_mix_params_flag: bool,
    has_common_curve_params_flag: bool,
    gain_application_space_chromaticities: [u16; 8],
    alternate_hdr_headrooms: [u16; 4],

    // Syntax elements of smpte_st_2094_50_component_mixing().
    component_mixing_type: [u8; 4],
    has_component_mixing_coefficient_flag: [[bool; 6]; 4],
    component_mixing_coefficient: [[u16; 6]; 4],

    // Syntax elements of smpte_st_2094_50_gain_curve().
    gain_curve_num_control_points_minus_1: [u8; 4],
    gain_curve_use_pchip_slope_flag: [bool; 4],
    gain_curve_control_points_x: [[u16; 32]; 4],
    gain_curve_control_points_y: [[u16; 32]; 4],
    gain_curve_control_points_theta: [[u16; 32]; 4],
}

impl AgtmSyntax {
    const NUM_CHROMATICITY_VALUES: usize = 8;
    const NUM_MIX_COEFFICIENTS: usize = 6;

    // Parse according to Clause C.2.
    // Port of: src/codec/SkHdrAgtmParse.cpp#L183-L196 (chrome/m156), parse_application_info
    fn parse_application_info(&mut self, s: &mut ByteReader<'_>) -> Option<()> {
        let mut flags = BitfieldReader::read_from(s)?;
        self.application_version = flags.read_bits(3);
        self.minimum_application_version = flags.read_bits(3);
        if self.minimum_application_version > 0 {
            return None;
        }
        let reserved_zero = flags.read_bits(2);
        if reserved_zero != 0 {
            return None;
        }
        self.parse_color_volume_transform(s)
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L198-L208 (chrome/m156), write_application_info
    fn write_application_info(&self, out: &mut Vec<u8>) {
        let mut flags = BitfieldWriter::default();
        flags.write_bits(self.application_version, 3);
        flags.write_bits(self.minimum_application_version, 3);
        flags.pad_and_write_to(out);
        self.write_color_volume_transform(out);
    }

    // Parse according to Table C.2.
    // Port of: src/codec/SkHdrAgtmParse.cpp#L210-L229 (chrome/m156), parse_color_volume_transform
    fn parse_color_volume_transform(&mut self, s: &mut ByteReader<'_>) -> Option<()> {
        let mut flags = BitfieldReader::read_from(s)?;
        self.has_custom_hdr_reference_white_flag = flags.read_flag();
        self.has_adaptive_tone_map_flag = flags.read_flag();
        let reserved_zero = flags.read_bits(6);
        if reserved_zero != 0 {
            return None;
        }
        if self.has_custom_hdr_reference_white_flag {
            self.hdr_reference_white = s.read_u16_be()?;
        }
        if self.has_adaptive_tone_map_flag {
            self.parse_adaptive_tone_map(s)?;
        }
        Some(())
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L231-L244 (chrome/m156), write_color_volume_transform
    fn write_color_volume_transform(&self, out: &mut Vec<u8>) {
        let mut flags = BitfieldWriter::default();
        flags.write_flag(self.has_custom_hdr_reference_white_flag);
        flags.write_flag(self.has_adaptive_tone_map_flag);
        flags.pad_and_write_to(out);
        if self.has_custom_hdr_reference_white_flag {
            out.extend_from_slice(&self.hdr_reference_white.to_be_bytes());
        }
        if self.has_adaptive_tone_map_flag {
            self.write_adaptive_tone_map(out);
        }
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L246-L279 (chrome/m156), parse_adaptive_tone_map
    fn parse_adaptive_tone_map(&mut self, s: &mut ByteReader<'_>) -> Option<()> {
        self.baseline_hdr_headroom = s.read_u16_be()?;
        let mut flags = BitfieldReader::read_from(s)?;
        self.use_reference_white_tone_mapping_flag = flags.read_flag();
        if !self.use_reference_white_tone_mapping_flag {
            self.num_alternate_images = flags.read_bits(3);
            self.gain_application_space_chromaticities_flag = flags.read_bits(2);
            self.has_common_component_mix_params_flag = flags.read_flag();
            self.has_common_curve_params_flag = flags.read_flag();
            if self.gain_application_space_chromaticities_flag == 3 {
                for r in 0..Self::NUM_CHROMATICITY_VALUES {
                    self.gain_application_space_chromaticities[r] = s.read_u16_be()?;
                }
            }
            let count = usize::from(self.num_alternate_images)
                .min(HeadroomAdaptiveToneMap::MAX_NUM_ALTERNATE_IMAGES);
            for a in 0..count {
                self.alternate_hdr_headrooms[a] = s.read_u16_be()?;
                self.parse_component_mixing(a, s)?;
                self.parse_gain_curve(a, s)?;
            }
        } else {
            let reserved_zero = flags.read_bits(7);
            if reserved_zero != 0 {
                return None;
            }
        }
        Some(())
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L281-L301 (chrome/m156), write_adaptive_tone_map
    fn write_adaptive_tone_map(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.baseline_hdr_headroom.to_be_bytes());
        let mut flags = BitfieldWriter::default();
        flags.write_flag(self.use_reference_white_tone_mapping_flag);
        if !self.use_reference_white_tone_mapping_flag {
            flags.write_bits(self.num_alternate_images, 3);
            flags.write_bits(self.gain_application_space_chromaticities_flag, 2);
            flags.write_flag(self.has_common_component_mix_params_flag);
            flags.write_flag(self.has_common_curve_params_flag);
            flags.pad_and_write_to(out);
            if self.gain_application_space_chromaticities_flag == 3 {
                for r in 0..Self::NUM_CHROMATICITY_VALUES {
                    out.extend_from_slice(
                        &self.gain_application_space_chromaticities[r].to_be_bytes(),
                    );
                }
            }
            for a in 0..usize::from(self.num_alternate_images) {
                out.extend_from_slice(&self.alternate_hdr_headrooms[a].to_be_bytes());
                self.write_component_mixing(a, out);
                self.write_gain_curve(a, out);
            }
        } else {
            flags.pad_and_write_to(out);
        }
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L303-L332 (chrome/m156), parse_component_mixing
    fn parse_component_mixing(&mut self, a: usize, s: &mut ByteReader<'_>) -> Option<()> {
        if a == 0 || !self.has_common_component_mix_params_flag {
            let mut flags = BitfieldReader::read_from(s)?;
            self.component_mixing_type[a] = flags.read_bits(2);
            if self.component_mixing_type[a] != 3 {
                let reserved_zero = flags.read_bits(Self::NUM_MIX_COEFFICIENTS as u32);
                if reserved_zero != 0 {
                    return None;
                }
            } else {
                for k in 0..Self::NUM_MIX_COEFFICIENTS {
                    self.has_component_mixing_coefficient_flag[a][k] = flags.read_flag();
                }
                for k in 0..Self::NUM_MIX_COEFFICIENTS {
                    if self.has_component_mixing_coefficient_flag[a][k] {
                        self.component_mixing_coefficient[a][k] = s.read_u16_be()?;
                    } else {
                        self.component_mixing_coefficient[a][k] = 0;
                    }
                }
            }
        } else {
            self.component_mixing_type[a] = self.component_mixing_type[0];
            if self.component_mixing_type[a] == 3 {
                for k in 0..Self::NUM_MIX_COEFFICIENTS {
                    self.component_mixing_coefficient[a][k] =
                        self.component_mixing_coefficient[0][k];
                }
            }
        }
        Some(())
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L334-L356 (chrome/m156), write_component_mixing
    fn write_component_mixing(&self, a: usize, out: &mut Vec<u8>) {
        if a == 0 || !self.has_common_component_mix_params_flag {
            let mut flags = BitfieldWriter::default();
            flags.write_bits(self.component_mixing_type[a], 2);
            if self.component_mixing_type[a] == 3 {
                for k in 0..Self::NUM_MIX_COEFFICIENTS {
                    flags.write_flag(self.has_component_mixing_coefficient_flag[a][k]);
                }
                flags.pad_and_write_to(out);
                for k in 0..Self::NUM_MIX_COEFFICIENTS {
                    if self.has_component_mixing_coefficient_flag[a][k] {
                        out.extend_from_slice(
                            &self.component_mixing_coefficient[a][k].to_be_bytes(),
                        );
                    }
                }
            } else {
                flags.pad_and_write_to(out);
            }
        }
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L358-L384 (chrome/m156), parse_gain_curve
    fn parse_gain_curve(&mut self, a: usize, s: &mut ByteReader<'_>) -> Option<()> {
        if a == 0 || !self.has_common_curve_params_flag {
            let mut flags = BitfieldReader::read_from(s)?;
            self.gain_curve_num_control_points_minus_1[a] = flags.read_bits(5);
            self.gain_curve_use_pchip_slope_flag[a] = flags.read_flag();
            let reserved_zero = flags.read_bits(2);
            if reserved_zero != 0 {
                return None;
            }
            for c in 0..=usize::from(self.gain_curve_num_control_points_minus_1[a]) {
                self.gain_curve_control_points_x[a][c] = s.read_u16_be()?;
            }
        } else {
            self.gain_curve_num_control_points_minus_1[a] =
                self.gain_curve_num_control_points_minus_1[0];
            self.gain_curve_use_pchip_slope_flag[a] = self.gain_curve_use_pchip_slope_flag[0];
            for c in 0..=usize::from(self.gain_curve_num_control_points_minus_1[a]) {
                self.gain_curve_control_points_x[a][c] = self.gain_curve_control_points_x[0][c];
            }
        }
        for c in 0..=usize::from(self.gain_curve_num_control_points_minus_1[a]) {
            self.gain_curve_control_points_y[a][c] = s.read_u16_be()?;
        }
        if !self.gain_curve_use_pchip_slope_flag[a] {
            for c in 0..=usize::from(self.gain_curve_num_control_points_minus_1[a]) {
                self.gain_curve_control_points_theta[a][c] = s.read_u16_be()?;
            }
        }
        Some(())
    }

    // Port of: src/codec/SkHdrAgtmParse.cpp#L386-L409 (chrome/m156), write_gain_curve
    fn write_gain_curve(&self, a: usize, out: &mut Vec<u8>) {
        // The C++ source tests has_common_component_mix_params_flag here, not the curve flag.
        // Keep that: it is what the encoder writes.
        if a == 0 || !self.has_common_component_mix_params_flag {
            let mut flags = BitfieldWriter::default();
            flags.write_bits(self.gain_curve_num_control_points_minus_1[a], 5);
            flags.write_flag(self.gain_curve_use_pchip_slope_flag[a]);
            flags.pad_and_write_to(out);
            for c in 0..=usize::from(self.gain_curve_num_control_points_minus_1[a]) {
                out.extend_from_slice(&self.gain_curve_control_points_x[a][c].to_be_bytes());
            }
        }
        for c in 0..=usize::from(self.gain_curve_num_control_points_minus_1[a]) {
            out.extend_from_slice(&self.gain_curve_control_points_y[a][c].to_be_bytes());
        }
        if !self.gain_curve_use_pchip_slope_flag[a] {
            for c in 0..=usize::from(self.gain_curve_num_control_points_minus_1[a]) {
                out.extend_from_slice(&self.gain_curve_control_points_theta[a][c].to_be_bytes());
            }
        }
    }
}

/// Structure containing all HDR metadata that can be attached to an image or video frame
/// (`skhdr::Metadata`).
// Port of: include/private/SkHdrMetadata.h#L363-L381 (chrome/m156), Metadata
#[doc(alias = "skhdr::Metadata")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    content_light_level_information: Option<ContentLightLevelInformation>,
    mastering_display_color_volume: Option<MasteringDisplayColorVolume>,
    adaptive_global_tone_map: Option<AdaptiveGlobalToneMap>,
}

impl Metadata {
    /// Returns a container with no metadata (`MakeEmpty`).
    // Port of: src/codec/SkHdrMetadata.cpp#L165-L167 (chrome/m156), MakeEmpty
    #[must_use]
    #[doc(alias = "MakeEmpty")]
    pub fn make_empty() -> Self {
        Self::default()
    }

    /// The content light level information, or `None` if there is none.
    // Port of: src/codec/SkHdrMetadata.cpp#L169-L177 (chrome/m156), getContentLightLevelInformation
    #[must_use]
    pub fn get_content_light_level_information(&self) -> Option<ContentLightLevelInformation> {
        self.content_light_level_information
    }

    /// Sets the content light level information.
    // Port of: src/codec/SkHdrMetadata.cpp#L226-L228 (chrome/m156), setContentLightLevelInformation
    pub fn set_content_light_level_information(&mut self, clli: ContentLightLevelInformation) {
        self.content_light_level_information = Some(clli);
    }

    /// The mastering display colour volume, or `None` if there is none.
    // Port of: src/codec/SkHdrMetadata.cpp#L179-L187 (chrome/m156), getMasteringDisplayColorVolume
    #[must_use]
    pub fn get_mastering_display_color_volume(&self) -> Option<MasteringDisplayColorVolume> {
        self.mastering_display_color_volume.clone()
    }

    /// Sets the mastering display colour volume.
    // Port of: src/codec/SkHdrMetadata.cpp#L221-L224 (chrome/m156), setMasteringDisplayColorVolume
    pub fn set_mastering_display_color_volume(&mut self, mdcv: MasteringDisplayColorVolume) {
        self.mastering_display_color_volume = Some(mdcv);
    }

    /// The adaptive global tone map, or `None` if there is none.
    // Port of: src/codec/SkHdrMetadata.cpp#L189-L197 (chrome/m156), getAdaptiveGlobalToneMap
    #[must_use]
    pub fn get_adaptive_global_tone_map(&self) -> Option<AdaptiveGlobalToneMap> {
        self.adaptive_global_tone_map.clone()
    }

    /// Sets the adaptive global tone map.
    // Port of: src/codec/SkHdrMetadata.cpp#L230-L232 (chrome/m156), setAdaptiveGlobalToneMap
    pub fn set_adaptive_global_tone_map(&mut self, agtm: AdaptiveGlobalToneMap) {
        self.adaptive_global_tone_map = Some(agtm);
    }

    /// The serialized adaptive global tone map, or `None` if none has been set.
    // Port of: src/codec/SkHdrMetadata.cpp#L199-L203 (chrome/m156), getSerializedAgtm
    #[must_use]
    pub fn get_serialized_agtm(&self) -> Option<Data> {
        self.adaptive_global_tone_map
            .as_ref()
            .and_then(AdaptiveGlobalToneMap::serialize)
    }

    /// Sets the adaptive global tone map from its serialized encoding. An encoding that does not
    /// parse clears the metadata.
    // Port of: src/codec/SkHdrMetadata.cpp#L205-L213 (chrome/m156), setSerializedAgtm
    pub fn set_serialized_agtm(&mut self, agtm_encoded: Option<&Data>) {
        self.adaptive_global_tone_map = agtm_encoded.and_then(AdaptiveGlobalToneMap::parse);
    }
}
