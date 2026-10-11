// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/private/SkHdrMetadata.h#L26-L142 and src/codec/SkHdrMetadata.cpp#L17-L164
// (chrome/m156): the content light level information (`skhdr::ContentLightLevelInformation`) and
// the mastering display colour volume (`skhdr::MasteringDisplayColorVolume`), each with its
// binary encodings. The `toString` helpers are not ported: they only format text.

//! HDR metadata: the content light level and mastering display colour volume.

use crate::color_space::ColorSpacePrimaries;
use crate::data::Data;

/// The PNG `cLLI` chunk stores luminance as an integer, the value times this (`clli_png_luminance_divisor`).
// Port of: src/codec/SkHdrMetadata.cpp#L48-L49 (chrome/m156), clli_png_luminance_divisor
const CLLI_PNG_LUMINANCE_DIVISOR: f32 = 10000.0;

/// The `mDCV` chunk stores chromaticity as an integer, the value times this
/// (`mdcv_chrominance_divisor`).
// Port of: src/codec/SkHdrMetadata.cpp#L91-L92 (chrome/m156), mdcv_chrominance_divisor
const MDCV_CHROMINANCE_DIVISOR: f32 = 50000.0;

/// The `mDCV` chunk stores luminance as an integer, the value times this
/// (`mdcv_luminance_divisor`).
// Port of: src/codec/SkHdrMetadata.cpp#L92-L93 (chrome/m156), mdcv_luminance_divisor
const MDCV_LUMINANCE_DIVISOR: f32 = 10000.0;

/// Port of `std::llroundf`: the nearest integer, halfway cases away from zero. The result is
/// converted to a 64-bit integer, which the callers truncate as C++ does.
#[allow(clippy::cast_possible_truncation)] // the value is already rounded, as llroundf gives
fn llroundf(value: f32) -> i64 {
    value.round() as i64
}

/// Content light level metadata (`skhdr::ContentLightLevelInformation`).
///
/// The semantics are defined in ANSI/CTA-861-H Annex P, and, with slightly different encodings,
/// in the PNG specification's `cLLI` chunk. Only the ways that work with both are used.
// Port of: include/private/SkHdrMetadata.h#L26-L96 (chrome/m156), ContentLightLevelInformation
#[doc(alias = "skhdr::ContentLightLevelInformation")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContentLightLevelInformation {
    /// The maximum content light level (`fMaxCLL`), in nits.
    pub max_cll: f32,
    /// The maximum frame-average light level (`fMaxFALL`), in nits.
    pub max_fall: f32,
}

impl ContentLightLevelInformation {
    /// The values as `uint16_t`, which the CTA encodings use (`MakeUint16`).
    // Port of: include/private/SkHdrMetadata.h#L61-L63 (chrome/m156), MakeUint16
    #[must_use]
    pub fn make_uint16(max_cll: u16, max_fall: u16) -> Self {
        Self {
            max_cll: f32::from(max_cll),
            max_fall: f32::from(max_fall),
        }
    }

    /// The maximum content light level, clamped and rounded to a `uint16_t`
    /// (`getUint16MaxCLL`).
    // Port of: include/private/SkHdrMetadata.h#L64-L66 (chrome/m156), getUint16MaxCLL
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 0..=65535
    pub fn get_uint16_max_cll(&self) -> u16 {
        self.max_cll.round().clamp(0.0, 65535.0) as u16
    }

    /// The maximum frame-average light level, clamped and rounded to a `uint16_t`
    /// (`getUint16MaxFALL`).
    // Port of: include/private/SkHdrMetadata.h#L67-L69 (chrome/m156), getUint16MaxFALL
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 0..=65535
    pub fn get_uint16_max_fall(&self) -> u16 {
        self.max_fall.round().clamp(0.0, 65535.0) as u16
    }

    /// Decodes the binary encoding of AV1 and H.265 (four bytes: two big-endian `uint16_t`). Returns
    /// `None` if the data is not that size.
    // Port of: src/codec/SkHdrMetadata.cpp#L21-L39 (chrome/m156), parse
    #[must_use]
    pub fn parse(data: &Data) -> Option<Self> {
        let bytes = data.as_bytes();
        if bytes.len() != 4 {
            return None;
        }
        let max_cll = u16::from_be_bytes([bytes[0], bytes[1]]);
        let max_fall = u16::from_be_bytes([bytes[2], bytes[3]]);
        Some(Self {
            max_cll: f32::from(max_cll),
            max_fall: f32::from(max_fall),
        })
    }

    /// Serializes to the encoding that [`parse`](Self::parse) reads.
    // Port of: src/codec/SkHdrMetadata.cpp#L41-L46 (chrome/m156), serialize
    #[must_use]
    pub fn serialize(&self) -> Data {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&self.get_uint16_max_cll().to_be_bytes());
        out.extend_from_slice(&self.get_uint16_max_fall().to_be_bytes());
        Data::new_from_vec(out)
    }

    /// Decodes the PNG `cLLI` chunk: two big-endian `uint32_t` values in units of 1/10000 nit.
    /// Returns `None` if the data is not eight bytes.
    // Port of: src/codec/SkHdrMetadata.cpp#L52-L70 (chrome/m156), parsePngChunk
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // the C++ converts the uint32_t to float
    pub fn parse_png_chunk(data: &Data) -> Option<Self> {
        let bytes = data.as_bytes();
        if bytes.len() != 8 {
            return None;
        }
        let max_cll_times_10000 = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let max_fall_times_10000 = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        Some(Self {
            max_cll: max_cll_times_10000 as f32 / CLLI_PNG_LUMINANCE_DIVISOR,
            max_fall: max_fall_times_10000 as f32 / CLLI_PNG_LUMINANCE_DIVISOR,
        })
    }

    /// Serializes to the encoding that [`parse_png_chunk`](Self::parse_png_chunk) reads.
    // Port of: src/codec/SkHdrMetadata.cpp#L72-L77 (chrome/m156), serializePngChunk
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ writes uint32_t
    pub fn serialize_png_chunk(&self) -> Data {
        let max_cll = llroundf(self.max_cll * CLLI_PNG_LUMINANCE_DIVISOR) as u32;
        let max_fall = llroundf(self.max_fall * CLLI_PNG_LUMINANCE_DIVISOR) as u32;
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&max_cll.to_be_bytes());
        out.extend_from_slice(&max_fall.to_be_bytes());
        Data::new_from_vec(out)
    }
}

/// Mastering display colour volume metadata (`skhdr::MasteringDisplayColorVolume`), from SMPTE
/// ST 2086:2018.
// Port of: include/private/SkHdrMetadata.h#L98-L140 (chrome/m156), MasteringDisplayColorVolume
#[doc(alias = "skhdr::MasteringDisplayColorVolume")]
#[derive(Clone, Debug, PartialEq)]
pub struct MasteringDisplayColorVolume {
    /// The chromaticities of the display (`fDisplayPrimaries`).
    pub display_primaries: ColorSpacePrimaries,
    /// The maximum luminance of the display, in nits (`fMaximumDisplayMasteringLuminance`).
    pub maximum_display_mastering_luminance: f32,
    /// The minimum luminance of the display, in nits (`fMinimumDisplayMasteringLuminance`).
    pub minimum_display_mastering_luminance: f32,
}

impl Default for MasteringDisplayColorVolume {
    // Port of: include/private/SkHdrMetadata.h#L99-L101 (chrome/m156), the in-class initializers
    fn default() -> Self {
        Self {
            display_primaries: ColorSpacePrimaries {
                rx: 0.0,
                ry: 0.0,
                gx: 0.0,
                gy: 0.0,
                bx: 0.0,
                by: 0.0,
                wx: 0.0,
                wy: 0.0,
            },
            maximum_display_mastering_luminance: 0.0,
            minimum_display_mastering_luminance: 0.0,
        }
    }
}

impl MasteringDisplayColorVolume {
    /// Decodes the binary encoding of AV1, H.265 and the PNG `mDCV` chunk: eight big-endian
    /// `uint16_t` chromaticities, then two big-endian `uint32_t` luminances. Returns `None` if the
    /// data is not 24 bytes.
    // Port of: src/codec/SkHdrMetadata.cpp#L100-L134 (chrome/m156), parse
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // the C++ converts the integers to float
    pub fn parse(data: &Data) -> Option<Self> {
        let bytes = data.as_bytes();
        if bytes.len() != 24 {
            return None;
        }
        let mut chromaticities = [0.0f32; 8];
        for (i, chromaticity) in chromaticities.iter_mut().enumerate() {
            let value = u16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]);
            *chromaticity = f32::from(value) / MDCV_CHROMINANCE_DIVISOR;
        }
        let max_luminance_times_10000 =
            u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let min_luminance_times_10000 =
            u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        Some(Self {
            display_primaries: ColorSpacePrimaries {
                rx: chromaticities[0],
                ry: chromaticities[1],
                gx: chromaticities[2],
                gy: chromaticities[3],
                bx: chromaticities[4],
                by: chromaticities[5],
                wx: chromaticities[6],
                wy: chromaticities[7],
            },
            maximum_display_mastering_luminance: max_luminance_times_10000 as f32
                / MDCV_LUMINANCE_DIVISOR,
            minimum_display_mastering_luminance: min_luminance_times_10000 as f32
                / MDCV_LUMINANCE_DIVISOR,
        })
    }

    /// Serializes to the encoding that [`parse`](Self::parse) reads.
    // Port of: src/codec/SkHdrMetadata.cpp#L136-L151 (chrome/m156), serialize
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ writes uint16_t/uint32_t
    pub fn serialize(&self) -> Data {
        let p = &self.display_primaries;
        let chromaticities = [p.rx, p.ry, p.gx, p.gy, p.bx, p.by, p.wx, p.wy];
        let mut out = Vec::with_capacity(24);
        for chromaticity in chromaticities {
            let value = llroundf(chromaticity * MDCV_CHROMINANCE_DIVISOR) as u16;
            out.extend_from_slice(&value.to_be_bytes());
        }
        let max_luminance =
            llroundf(self.maximum_display_mastering_luminance * MDCV_LUMINANCE_DIVISOR) as u32;
        let min_luminance =
            llroundf(self.minimum_display_mastering_luminance * MDCV_LUMINANCE_DIVISOR) as u32;
        out.extend_from_slice(&max_luminance.to_be_bytes());
        out.extend_from_slice(&min_luminance.to_be_bytes());
        Data::new_from_vec(out)
    }
}
