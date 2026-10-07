// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkColorSpace.h, src/core/SkColorSpace.cpp,
//                   src/core/SkColorSpacePriv.h

//! [`ColorSpace`]: a transfer function plus a gamut (a 3x3 transform to XYZ D50), a cheaply
//! clonable `Arc`-backed handle.

use std::fmt;
use std::sync::{Arc, LazyLock, OnceLock};

use skia_rust_skcms::{self as skcms, IccProfile, Matrix3x3, TfType, TransferFunction};

use crate::checksum::hash32;
use crate::color_space_priv::{
    color_space_almost_equal, is_almost_2dot2, is_almost_linear, is_almost_srgb,
};

/// Alias of skcms's [`TransferFunction`], named as in `skia-safe`.
#[doc(alias = "skcms_TransferFunction")]
pub type ColorSpaceTransferFn = TransferFunction;

/// Describes a color gamut with primaries and a white point.
// Port of: include/core/SkColorSpace.h#L25-L45 (chrome/m156)
#[doc(alias = "SkColorSpacePrimaries")]
#[derive(Clone, Debug, PartialEq)]
pub struct ColorSpacePrimaries {
    pub rx: f32,
    pub ry: f32,
    pub gx: f32,
    pub gy: f32,
    pub bx: f32,
    pub by: f32,
    pub wx: f32,
    pub wy: f32,
}

impl ColorSpacePrimaries {
    /// Creates primaries from the red, green, blue and white point chromaticities.
    #[must_use]
    #[allow(clippy::too_many_arguments)] // eight chromaticity coordinates
    pub const fn new(
        rx: f32,
        ry: f32,
        gx: f32,
        gy: f32,
        bx: f32,
        by: f32,
        wx: f32,
        wy: f32,
    ) -> Self {
        Self {
            rx,
            ry,
            gx,
            gy,
            bx,
            by,
            wx,
            wy,
        }
    }

    /// Converts primaries and a white point to a `toXYZD50` matrix, the preferred color gamut
    /// representation of [`ColorSpace`].
    // Port of: src/core/SkColorSpace.cpp#L126-L128 (chrome/m156)
    #[doc(alias = "toXYZD50")]
    #[must_use]
    pub fn to_xyzd50(&self) -> Option<Matrix3x3> {
        skcms::primaries_to_xyzd50(
            self.rx, self.ry, self.gx, self.gy, self.bx, self.by, self.wx, self.wy,
        )
    }
}

/// Color primaries defined by ITU-T H.273, table 2. Names are given by the first
/// specification referenced in the value's row.
// Port of: include/core/SkColorSpace.h#L51-L128 (chrome/m156)
#[doc(alias = "SkNamedPrimaries")]
pub mod named_primaries {
    use super::ColorSpacePrimaries;
    use skia_rust_skcms::Matrix3x3;

    /// Rec. ITU-R BT.709-6, value 1.
    #[doc(alias = "kRec709")]
    pub const REC709: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.64, 0.33, 0.3, 0.6, 0.15, 0.06, 0.3127, 0.329);

    /// Rec. ITU-R BT.470-6 System M (historical), value 4.
    #[doc(alias = "kRec470SystemM")]
    pub const REC470_SYSTEM_M: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.67, 0.33, 0.21, 0.71, 0.14, 0.08, 0.31, 0.316);

    /// Rec. ITU-R BT.470-6 System B, G (historical), value 5.
    #[doc(alias = "kRec470SystemBG")]
    pub const REC470_SYSTEM_BG: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.64, 0.33, 0.29, 0.60, 0.15, 0.06, 0.3127, 0.3290);

    /// Rec. ITU-R BT.601-7 525, value 6.
    #[doc(alias = "kRec601")]
    pub const REC601: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.630, 0.340, 0.310, 0.595, 0.155, 0.070, 0.3127, 0.3290);

    /// SMPTE ST 240, value 7 (functionally the same as value 6).
    #[doc(alias = "kSMPTE_ST_240")]
    pub const SMPTE_ST_240: ColorSpacePrimaries = REC601;

    /// Generic film (colour filters using Illuminant C), value 8.
    #[doc(alias = "kGenericFilm")]
    pub const GENERIC_FILM: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.681, 0.319, 0.243, 0.692, 0.145, 0.049, 0.310, 0.316);

    /// Rec. ITU-R BT.2020-2, value 9.
    #[doc(alias = "kRec2020")]
    pub const REC2020: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.708, 0.292, 0.170, 0.797, 0.131, 0.046, 0.3127, 0.3290);

    /// SMPTE ST 428-1, value 10.
    #[doc(alias = "kSMPTE_ST_428_1")]
    pub const SMPTE_ST_428_1: ColorSpacePrimaries =
        ColorSpacePrimaries::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0 / 3.0, 1.0 / 3.0);

    /// SMPTE RP 431-2, value 11.
    #[doc(alias = "kSMPTE_RP_431_2")]
    pub const SMPTE_RP_431_2: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.314, 0.351);

    /// SMPTE EG 432-1, value 12.
    #[doc(alias = "kSMPTE_EG_432_1")]
    pub const SMPTE_EG_432_1: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290);

    /// No corresponding industry specification identified, value 22.
    /// This is sometimes referred to as EBU 3213-E, but that document doesn't
    /// specify these values.
    #[doc(alias = "kITU_T_H273_Value22")]
    pub const ITU_T_H273_VALUE22: ColorSpacePrimaries =
        ColorSpacePrimaries::new(0.630, 0.340, 0.295, 0.605, 0.155, 0.077, 0.3127, 0.3290);

    /// Mapping between names of color primaries and the number of the corresponding
    /// row in ITU-T H.273, table 2.  As above, the constants are named based on the
    /// first specification referenced in the value's row.
    #[doc(alias = "SkNamedPrimaries::CicpId")]
    #[allow(non_camel_case_types)] // variant names follow skia-safe (and Skia's k-prefixed names)
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u8)]
    pub enum CicpId {
        // Value 0 is reserved.
        Rec709 = 1,
        // Value 2 is unspecified.
        // Value 3 is reserved.
        Rec470SystemM = 4,
        Rec470SystemBG = 5,
        Rec601 = 6,
        SMPTE_ST_240 = 7,
        GenericFilm = 8,
        Rec2020 = 9,
        SMPTE_ST_428_1 = 10,
        SMPTE_RP_431_2 = 11,
        SMPTE_EG_432_1 = 12,
        // Values 13-21 are reserved.
        ITU_T_H273_Value22 = 22,
        // Values 23-255 are reserved.
    }

    impl CicpId {
        /// The `CicpId` with the given H.273 value, if there is one. (A C++ `CicpId` can hold
        /// any `uint8_t`; a Rust enum cannot.)
        #[must_use]
        pub fn from_u8(value: u8) -> Option<Self> {
            Some(match value {
                1 => Self::Rec709,
                4 => Self::Rec470SystemM,
                5 => Self::Rec470SystemBG,
                6 => Self::Rec601,
                7 => Self::SMPTE_ST_240,
                8 => Self::GenericFilm,
                9 => Self::Rec2020,
                10 => Self::SMPTE_ST_428_1,
                11 => Self::SMPTE_RP_431_2,
                12 => Self::SMPTE_EG_432_1,
                22 => Self::ITU_T_H273_Value22,
                _ => return None,
            })
        }
    }

    /// <https://www.w3.org/TR/css-color-4/#predefined-prophoto-rgb>
    #[doc(alias = "kProPhotoRGB")]
    pub const PRO_PHOTO_RGB: ColorSpacePrimaries = ColorSpacePrimaries::new(
        0.7347, 0.2653, 0.1596, 0.8404, 0.0366, 0.0001, 0.34567, 0.35850,
    );

    /// Value 2 indicates "characteristics are unknown or are determined by the application". In
    /// practice, this means we will delegate the primaries to the toXYZD50 tags.
    // Port of: src/core/SkColorSpace.cpp#L53-L54 (chrome/m156)
    pub const CICP_ID_APPLICATION_DEFINED: u8 = 2;

    // Rec. ITU-T H.273, Table 2.
    // Port of: src/core/SkColorSpace.cpp#L33-L52 (chrome/m156)
    struct TableEntry {
        cicp_id: CicpId,
        sk_primaries: ColorSpacePrimaries,
        to_xyzd50: Option<Matrix3x3>,
    }

    fn cicp_table() -> [TableEntry; 11] {
        use super::named_gamut;
        [
            TableEntry {
                cicp_id: CicpId::Rec709,
                sk_primaries: REC709,
                to_xyzd50: Some(named_gamut::SRGB),
            },
            TableEntry {
                cicp_id: CicpId::Rec470SystemM,
                sk_primaries: REC470_SYSTEM_M,
                to_xyzd50: None,
            },
            TableEntry {
                cicp_id: CicpId::Rec470SystemBG,
                sk_primaries: REC470_SYSTEM_BG,
                to_xyzd50: None,
            },
            TableEntry {
                cicp_id: CicpId::Rec601,
                sk_primaries: REC601,
                to_xyzd50: None,
            },
            TableEntry {
                cicp_id: CicpId::SMPTE_ST_240,
                sk_primaries: SMPTE_ST_240,
                to_xyzd50: None,
            },
            TableEntry {
                cicp_id: CicpId::GenericFilm,
                sk_primaries: GENERIC_FILM,
                to_xyzd50: None,
            },
            TableEntry {
                cicp_id: CicpId::Rec2020,
                sk_primaries: REC2020,
                to_xyzd50: Some(named_gamut::REC2020),
            },
            TableEntry {
                cicp_id: CicpId::SMPTE_ST_428_1,
                sk_primaries: SMPTE_ST_428_1,
                to_xyzd50: None,
            },
            TableEntry {
                cicp_id: CicpId::SMPTE_RP_431_2,
                sk_primaries: SMPTE_RP_431_2,
                to_xyzd50: None,
            },
            TableEntry {
                cicp_id: CicpId::SMPTE_EG_432_1,
                sk_primaries: SMPTE_EG_432_1,
                to_xyzd50: Some(named_gamut::DISPLAY_P3),
            },
            TableEntry {
                cicp_id: CicpId::ITU_T_H273_Value22,
                sk_primaries: ITU_T_H273_VALUE22,
                to_xyzd50: None,
            },
        ]
    }

    /// The `toXYZD50` matrix of the named primaries, if known.
    // Port of: src/core/SkColorSpace.cpp#L57-L71 (chrome/m156)
    #[doc(alias = "GetCicp")]
    #[must_use]
    pub fn get_cicp(primaries: CicpId) -> Option<Matrix3x3> {
        for table_entry in &cicp_table() {
            if primaries != table_entry.cicp_id {
                continue;
            }
            if let Some(m) = table_entry.to_xyzd50 {
                return Some(m);
            }
            if let Some(m) = table_entry.sk_primaries.to_xyzd50() {
                return Some(m);
            }
        }
        None
    }

    /// The named primaries whose `toXYZD50` matrix is almost equal to `m`, if any.
    // Port of: src/core/SkColorSpace.cpp#L73-L85 (chrome/m156)
    #[doc(alias = "GetCicpFromMatrix")]
    #[must_use]
    pub fn get_cicp_from_matrix(m: &Matrix3x3) -> Option<CicpId> {
        for table_entry in &cicp_table() {
            if let Some(table_entry_m) = table_entry.sk_primaries.to_xyzd50()
                && super::xyz_almost_equal(m, &table_entry_m)
            {
                return Some(table_entry.cicp_id);
            }
        }
        None
    }
}

/// Named [`ColorSpaceTransferFn`] constants for common transfer functions.
// Port of: include/core/SkColorSpace.h#L130-L230 (chrome/m156)
#[doc(alias = "SkNamedTransferFn")]
pub mod named_transfer_fn {
    use super::ColorSpaceTransferFn;

    /// Like `SkNamedGamut::kSRGB`, keeping this bitwise exactly the same as skcms makes things
    /// fastest.
    #[doc(alias = "kSRGB")]
    #[allow(clippy::cast_possible_truncation)] // mirrors the C++ (float)(double expression) casts
    pub const SRGB: ColorSpaceTransferFn = ColorSpaceTransferFn::new(
        2.4,
        (1.0f64 / 1.055) as f32,
        (0.055f64 / 1.055) as f32,
        (1.0f64 / 12.92) as f32,
        0.04045,
        0.0,
        0.0,
    );

    #[doc(alias = "k2Dot2")]
    pub const DOT22: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    #[doc(alias = "kRec2020")]
    pub const REC2020: ColorSpaceTransferFn = ColorSpaceTransferFn::new(
        2.22222,
        0.909_672,
        0.090_327_6,
        0.222_222,
        0.081_242_9,
        0.0,
        0.0,
    );

    // Transfer function defined by ITU-T H.273, table 3. Names are given by the
    // first specification referenced in the value's row. The equations in table 3
    // "either indicates the reference [OETF] ... or indicates the inverse of the
    // reference EOTF". The transfer functions provided are reference EOTFs.

    /// Rec. ITU-R BT.709-6, value 1. This follows note 1, which reads: "In the cases
    /// of [...] `TransferCharacteristics` equal to 1, 6, 14 or 15 [...], although the
    /// value is defined in terms of a reference [OETF], a suggested corresponding
    /// reference [EOTF] has been specified in Rec. ITU-R BT.1886-0."
    #[doc(alias = "kRec709")]
    pub const REC709: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(2.4, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    /// Rec. ITU-R BT.470-6 System M (historical) assumed display gamma 2.2, value 4.
    #[doc(alias = "kRec470SystemM")]
    pub const REC470_SYSTEM_M: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    /// Rec. ITU-R BT.470-6 System B, G (historical) assumed display gamma 2.8, value 5.
    #[doc(alias = "kRec470SystemBG")]
    pub const REC470_SYSTEM_BG: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(2.8, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    /// Rec. ITU-R BT.601-7, same as `REC709`, value 6.
    #[doc(alias = "kRec601")]
    pub const REC601: ColorSpaceTransferFn = REC709;

    /// SMPTE ST 240, value 7.
    #[doc(alias = "kSMPTE_ST_240")]
    #[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
    pub const SMPTE_ST_240: ColorSpaceTransferFn = ColorSpaceTransferFn::new(
        2.222_222_222_222,
        0.899_626_676_224,
        0.100_373_323_776,
        0.25,
        0.091_286_342_118,
        0.0,
        0.0,
    );

    /// Linear, value 8
    #[doc(alias = "kLinear")]
    pub const LINEAR: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    /// IEC 61966-2-4, value 11, same as `REC709` (but is explicitly extended).
    #[doc(alias = "kIEC61966_2_4")]
    pub const IEC61966_2_4: ColorSpaceTransferFn = REC709;

    /// IEC 61966-2-1 sRGB, value 13.
    #[doc(alias = "kIEC61966_2_1")]
    pub const IEC61966_2_1: ColorSpaceTransferFn = SRGB;

    /// Rec. ITU-R BT.2020-2 (10-bit system), value 14.
    #[doc(alias = "kRec2020_10bit")]
    pub const REC2020_10BIT: ColorSpaceTransferFn = REC709;

    /// Rec. ITU-R BT.2020-2 (12-bit system), value 15.
    #[doc(alias = "kRec2020_12bit")]
    pub const REC2020_12BIT: ColorSpaceTransferFn = REC709;

    /// Rec. ITU-R BT.2100-2 perceptual quantization (PQ) system, value 16.
    #[doc(alias = "kPQ")]
    pub const PQ: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(-5.0, 203.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    /// SMPTE ST 428-1, value 17.
    #[doc(alias = "kSMPTE_ST_428_1")]
    #[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
    pub const SMPTE_ST_428_1: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(2.6, 1.034_080_527_699, 0.0, 0.0, 0.0, 0.0, 0.0);

    /// Rec. ITU-R BT.2100-2 hybrid log-gamma (HLG) system, value 18.
    #[doc(alias = "kHLG")]
    pub const HLG: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(-6.0, 203.0, 1000.0, 1.2, 0.0, 0.0, 0.0);

    /// Mapping between transfer function names and the number of the corresponding
    /// row in ITU-T H.273, table 3.  As above, the constants are named based on the
    /// first specification referenced in the value's row.
    #[doc(alias = "SkNamedTransferFn::CicpId")]
    #[allow(non_camel_case_types)] // variant names follow skia-safe (and Skia's k-prefixed names)
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u8)]
    pub enum CicpId {
        // Value 0 is reserved.
        Rec709 = 1,
        // Value 2 is unspecified.
        // Value 3 is reserved.
        Rec470SystemM = 4,
        Rec470SystemBG = 5,
        Rec601 = 6,
        SMPTE_ST_240 = 7,
        Linear = 8,
        // Value 9 is not supported by `ColorSpace::new_cicp`.
        // Value 10 is not supported by `ColorSpace::new_cicp`.
        IEC61966_2_4 = 11,
        // Value 12 is not supported by `ColorSpace::new_cicp`.
        IEC61966_2_1 = 13,
        Rec2020_10bit = 14,
        Rec2020_12bit = 15,
        PQ = 16,
        SMPTE_ST_428_1 = 17,
        HLG = 18,
        // Values 19-255 are reserved.
    }

    impl CicpId {
        /// `kSRGB`, the same value as [`CicpId::IEC61966_2_1`].
        #[doc(alias = "kSRGB")]
        pub const SRGB: CicpId = CicpId::IEC61966_2_1;

        /// The `CicpId` with the given H.273 value, if there is one. (A C++ `CicpId` can hold
        /// any `uint8_t`; a Rust enum cannot.)
        #[must_use]
        pub fn from_u8(value: u8) -> Option<Self> {
            Some(match value {
                1 => Self::Rec709,
                4 => Self::Rec470SystemM,
                5 => Self::Rec470SystemBG,
                6 => Self::Rec601,
                7 => Self::SMPTE_ST_240,
                8 => Self::Linear,
                11 => Self::IEC61966_2_4,
                13 => Self::IEC61966_2_1,
                14 => Self::Rec2020_10bit,
                15 => Self::Rec2020_12bit,
                16 => Self::PQ,
                17 => Self::SMPTE_ST_428_1,
                18 => Self::HLG,
                _ => return None,
            })
        }
    }

    /// <https://w3.org/TR/css-color-4/#valdef-color-prophoto-rgb>
    /// "The transfer curve is a gamma function with a value of 1/1.8"
    #[doc(alias = "kProPhotoRGB")]
    pub const PRO_PHOTO_RGB: ColorSpaceTransferFn =
        ColorSpaceTransferFn::new(1.8, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    /// <https://www.w3.org/TR/css-color-4/#predefined-a98-rgb>
    #[doc(alias = "kA98RGB")]
    pub const A98_RGB: ColorSpaceTransferFn = DOT22;

    /// Value 2 indicates "characteristics are unknown or are determined by the application". In
    /// practice, this means we will delegate the primaries to the trc tags.
    // Port of: src/core/SkColorSpace.cpp#L111-L112 (chrome/m156)
    pub const CICP_ID_APPLICATION_DEFINED: u8 = 2;

    // Rec. ITU-T H.273, Table 3.
    // Port of: src/core/SkColorSpace.cpp#L88-L110 (chrome/m156)
    const CICP_TABLE: [(CicpId, ColorSpaceTransferFn); 13] = [
        (CicpId::Rec709, REC709),
        (CicpId::Rec470SystemM, REC470_SYSTEM_M),
        (CicpId::Rec470SystemBG, REC470_SYSTEM_BG),
        (CicpId::Rec601, REC601),
        (CicpId::SMPTE_ST_240, SMPTE_ST_240),
        (CicpId::Linear, LINEAR),
        (CicpId::IEC61966_2_4, IEC61966_2_4),
        (CicpId::IEC61966_2_1, IEC61966_2_1),
        (CicpId::Rec2020_10bit, REC2020_10BIT),
        (CicpId::Rec2020_12bit, REC2020_12BIT),
        (CicpId::PQ, PQ),
        (CicpId::SMPTE_ST_428_1, SMPTE_ST_428_1),
        (CicpId::HLG, HLG),
    ];

    /// The transfer function of the named transfer characteristics, if known.
    // Port of: src/core/SkColorSpace.cpp#L114-L122 (chrome/m156)
    #[doc(alias = "GetCicp")]
    #[must_use]
    pub fn get_cicp(transfer_characteristics: CicpId) -> Option<ColorSpaceTransferFn> {
        for (cicp_id, trfn) in &CICP_TABLE {
            if transfer_characteristics == *cicp_id {
                return Some(*trfn);
            }
        }
        None
    }
}

/// Named gamuts: `toXYZD50` matrices.
// Port of: include/core/SkColorSpace.h#L232-L272 (chrome/m156)
#[doc(alias = "SkNamedGamut")]
pub mod named_gamut {
    use super::Matrix3x3;
    use crate::fixed::fixed_to_float;

    // ICC fixed-point (16.16) representation, taken from skcms. Please keep them exactly in sync.
    // 0.436065674f, 0.385147095f, 0.143066406f,
    // 0.222488403f, 0.716873169f, 0.060607910f,
    // 0.013916016f, 0.097076416f, 0.714096069f,
    #[doc(alias = "kSRGB")]
    pub const SRGB: Matrix3x3 = Matrix3x3 {
        vals: [
            [
                fixed_to_float(0x6FA2),
                fixed_to_float(0x6299),
                fixed_to_float(0x24A0),
            ],
            [
                fixed_to_float(0x38F5),
                fixed_to_float(0xB785),
                fixed_to_float(0x0F84),
            ],
            [
                fixed_to_float(0x0390),
                fixed_to_float(0x18DA),
                fixed_to_float(0xB6CF),
            ],
        ],
    };

    // ICC fixed-point (16.16) repesentation of:
    // 0.60974, 0.20528, 0.14919,
    // 0.31111, 0.62567, 0.06322,
    // 0.01947, 0.06087, 0.74457,
    #[doc(alias = "kAdobeRGB")]
    pub const ADOBE_RGB: Matrix3x3 = Matrix3x3 {
        vals: [
            [
                fixed_to_float(0x9c18),
                fixed_to_float(0x348d),
                fixed_to_float(0x2631),
            ],
            [
                fixed_to_float(0x4fa5),
                fixed_to_float(0xa02c),
                fixed_to_float(0x102f),
            ],
            [
                fixed_to_float(0x04fc),
                fixed_to_float(0x0f95),
                fixed_to_float(0xbe9c),
            ],
        ],
    };

    #[doc(alias = "kDisplayP3")]
    pub const DISPLAY_P3: Matrix3x3 = Matrix3x3 {
        vals: [
            [0.515_102, 0.291_965, 0.157_153],
            [0.241_182, 0.692_236, 0.066_581_9],
            [-0.001_049_41, 0.041_881_8, 0.784_378],
        ],
    };

    #[doc(alias = "kRec2020")]
    pub const REC2020: Matrix3x3 = Matrix3x3 {
        vals: [
            [0.673_459, 0.165_661, 0.125_100],
            [0.279_033, 0.675_338, 0.045_628_8],
            [-0.001_931_39, 0.029_979_4, 0.797_162],
        ],
    };

    #[doc(alias = "kXYZ")]
    pub const XYZ: Matrix3x3 = Matrix3x3 {
        vals: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
}

// Port of: src/core/SkColorSpace.cpp#L20-L30 (chrome/m156)
fn xyz_almost_equal(m_a: &Matrix3x3, m_b: &Matrix3x3) -> bool {
    for r in 0..3 {
        for c in 0..3 {
            if !color_space_almost_equal(m_a.vals[r][c], m_b.vals[r][c]) {
                return false;
            }
        }
    }

    true
}

// Port of: src/core/SkColorSpace.cpp#L413-L418 (chrome/m156)
const K1_VERSION: u8 = 1; // Simple header (version tag) + 16 floats
const K_CURRENT_VERSION: u8 = K1_VERSION;

// Port of: src/core/SkColorSpace.cpp#L420-L427 (chrome/m156)
// A `ColorSpaceHeader` is four bytes: the version and three reserved zero bytes.
const COLOR_SPACE_HEADER_SIZE: usize = 4;

// The lazily computed destination-side fields: the inverse transfer function and the inverse
// gamut matrix.
#[derive(Debug)]
struct LazyDstFields {
    inv_transfer_fn: TransferFunction,
    from_xyzd50: Matrix3x3,
}

#[derive(Debug)]
struct Inner {
    transfer_fn_hash: u32,
    to_xyzd50_hash: u32,

    transfer_fn: TransferFunction,
    to_xyzd50: Matrix3x3,

    lazy_dst_fields: OnceLock<LazyDstFields>,
}

/// Describes a color space: a transfer function and a gamut, cheaply clonable (it shares its
/// data, like a ref-counted handle).
// Port of: include/core/SkColorSpace.h#L274-L432 (chrome/m156)
#[doc(alias = "SkColorSpace")]
#[derive(Clone)]
pub struct ColorSpace(Arc<Inner>);

impl fmt::Debug for ColorSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColorSpace")
            .field("transfer_fn", &self.0.transfer_fn)
            .field("to_xyzd50", &self.0.to_xyzd50)
            .finish()
    }
}

/// `SkColorSpace::Equals`: color spaces are equal if they are the same object, or their hashes
/// match.
impl PartialEq for ColorSpace {
    fn eq(&self, other: &Self) -> bool {
        ColorSpace::equals(Some(self), Some(other))
    }
}

/// A hash of the gamut transformation to XYZ D50, as returned by [`ColorSpace::to_xyzd50_hash`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct XYZD50Hash(pub u32);

static SRGB_SINGLETON: LazyLock<ColorSpace> =
    LazyLock::new(|| ColorSpace::new_unchecked(named_transfer_fn::SRGB, named_gamut::SRGB));

static SRGB_LINEAR_SINGLETON: LazyLock<ColorSpace> =
    LazyLock::new(|| ColorSpace::new_unchecked(named_transfer_fn::LINEAR, named_gamut::SRGB));

// Port of: src/core/SkColorSpace.cpp#L186-L190 (chrome/m156)
pub(crate) fn srgb_singleton() -> &'static ColorSpace {
    &SRGB_SINGLETON
}

// Port of: src/core/SkColorSpace.cpp#L192-L196 (chrome/m156)
pub(crate) fn srgb_linear_singleton() -> &'static ColorSpace {
    &SRGB_LINEAR_SINGLETON
}

impl ColorSpace {
    // Port of: src/core/SkColorSpace.cpp#L130-L136 (chrome/m156)
    fn new_unchecked(transfer_fn: TransferFunction, to_xyzd50: Matrix3x3) -> Self {
        let mut tf_bytes = [0u8; 28];
        tf_bytes.copy_from_slice(&transfer_fn.to_ne_bytes());
        let mut xyz_bytes = [0u8; 36];
        for (i, v) in to_xyzd50.vals.iter().flatten().enumerate() {
            xyz_bytes[4 * i..4 * i + 4].copy_from_slice(&v.to_ne_bytes());
        }
        ColorSpace(Arc::new(Inner {
            transfer_fn_hash: hash32(&tf_bytes, 0),
            to_xyzd50_hash: hash32(&xyz_bytes, 0),
            transfer_fn,
            to_xyzd50,
            lazy_dst_fields: OnceLock::new(),
        }))
    }

    /// Creates the sRGB color space.
    // Port of: src/core/SkColorSpace.cpp#L198-L200 (chrome/m156)
    #[doc(alias = "MakeSRGB")]
    #[must_use]
    pub fn new_srgb() -> Self {
        srgb_singleton().clone()
    }

    /// Creates a color space with the sRGB primaries, but a linear (1.0) gamma.
    // Port of: src/core/SkColorSpace.cpp#L202-L204 (chrome/m156)
    #[doc(alias = "MakeSRGBLinear")]
    #[must_use]
    pub fn new_srgb_linear() -> Self {
        srgb_linear_singleton().clone()
    }

    /// Creates a [`ColorSpace`] from a transfer function and a row-major 3x3 transformation to
    /// XYZ. Returns `None` if the transfer function is invalid.
    // Port of: src/core/SkColorSpace.cpp#L138-L161 (chrome/m156)
    #[doc(alias = "MakeRGB")]
    #[must_use]
    pub fn new_rgb(transfer_fn: &TransferFunction, to_xyz: &Matrix3x3) -> Option<Self> {
        if transfer_fn.tf_type() == TfType::Invalid {
            return None;
        }

        let mut tf = transfer_fn;

        if is_almost_srgb(transfer_fn) {
            if xyz_almost_equal(to_xyz, &named_gamut::SRGB) {
                return Some(Self::new_srgb());
            }
            tf = &named_transfer_fn::SRGB;
        } else if is_almost_2dot2(transfer_fn) {
            tf = &named_transfer_fn::DOT22;
        } else if is_almost_linear(transfer_fn) {
            if xyz_almost_equal(to_xyz, &named_gamut::SRGB) {
                return Some(Self::new_srgb_linear());
            }
            tf = &named_transfer_fn::LINEAR;
        }

        Some(Self::new_unchecked(*tf, *to_xyz))
    }

    /// Creates a [`ColorSpace`] from code points specified in Rec. ITU-T H.273. Returns `None`
    /// for an invalid or unsupported combination of code points.
    ///
    /// - `primaries` identifies an entry in Rec. ITU-T H.273, Table 2
    /// - `transfer_characteristics` identifies an entry in Rec. ITU-T H.273, Table 3
    ///
    /// [`ColorSpace`] (and the underlying [`IccProfile`]) only supports RGB color spaces and
    /// therefore this function does not take a `matrix_coefficients` parameter; the caller is
    /// expected to verify that `matrix_coefficients` is `0`.
    ///
    /// Narrow range images are extremely rare, so this function does not take a
    /// `video_full_range_flag`; the caller is expected to verify that it is `1` (indicating a
    /// full range image).
    // Port of: src/core/SkColorSpace.cpp#L163-L184 (chrome/m156)
    #[doc(alias = "MakeCICP")]
    #[must_use]
    pub fn new_cicp(
        primaries: named_primaries::CicpId,
        transfer_characteristics: named_transfer_fn::CicpId,
    ) -> Option<Self> {
        let trfn = named_transfer_fn::get_cicp(transfer_characteristics)?;
        let to_xyzd50 = named_primaries::get_cicp(primaries)?;

        Self::new_rgb(&trfn, &to_xyzd50)
    }

    /// Creates a [`ColorSpace`] from a parsed (skcms) ICC profile.
    // Port of: src/core/SkColorSpace.cpp#L333-L411 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    #[allow(clippy::too_many_lines)] // mirrors the structure of the C++ function
    #[allow(clippy::if_same_then_else)] // mirrors the two C++ branches that both return sRGB
    pub fn make(profile: &IccProfile) -> Option<Self> {
        // The CICP values are only valid for full-range, with no matrix.
        let use_cicp = profile.has_cicp
            && profile.cicp.matrix_coefficients == 0
            && profile.cicp.video_full_range_flag == 1;
        let cicp_color_primaries = named_primaries::CicpId::from_u8(profile.cicp.color_primaries);
        let cicp_transfer_characteristics =
            named_transfer_fn::CicpId::from_u8(profile.cicp.transfer_characteristics);

        // Early checks for exact sRGB matches.
        if use_cicp
            && cicp_color_primaries == Some(named_primaries::CicpId::Rec709)
            && cicp_transfer_characteristics == Some(named_transfer_fn::CicpId::IEC61966_2_4)
        {
            return Some(Self::new_srgb());
        } else if skcms::approximately_equal_profiles(profile, skcms::srgb_profile()) {
            return Some(Self::new_srgb());
        }

        // Set the toXYZD50 matrix, preferring CICP over the matrix itself.
        let mut to_xyzd50 = Matrix3x3::default();
        let mut has_set_to_xyzd50 = false;
        if use_cicp {
            if let Some(m) = cicp_color_primaries.and_then(named_primaries::get_cicp) {
                to_xyzd50 = m;
                has_set_to_xyzd50 = true;
            } else if profile.cicp.color_primaries != named_primaries::CICP_ID_APPLICATION_DEFINED {
                return None;
            }
        }
        if profile.has_to_xyzd50 && !has_set_to_xyzd50 {
            // TODO: can we save this work and skip lazily inverting the matrix later?
            to_xyzd50 = profile.to_xyzd50;
            if to_xyzd50.invert().is_some() {
                has_set_to_xyzd50 = true;
            }
        }
        if !has_set_to_xyzd50 {
            return None;
        }

        // Set the transfer function, preferring CICP over the curves.
        let mut trfn = TransferFunction::default();
        let mut has_set_trfn = false;
        if use_cicp {
            if let Some(tf) = cicp_transfer_characteristics.and_then(named_transfer_fn::get_cicp) {
                trfn = tf;
                has_set_trfn = true;
            } else if profile.cicp.transfer_characteristics
                != named_transfer_fn::CICP_ID_APPLICATION_DEFINED
            {
                return None;
            }
        }
        if profile.has_trc && !has_set_trfn {
            // We can't work with tables or mismatched parametric curves.
            let trc = &profile.trc;
            if let (
                skcms::Curve::Parametric(t0),
                skcms::Curve::Parametric(t1),
                skcms::Curve::Parametric(t2),
            ) = (&trc[0], &trc[1], &trc[2])
                && t0.bit_eq(t1)
                && t0.bit_eq(t2)
            {
                trfn = *t0;
                has_set_trfn = true;
            } else {
                // If all curves look close enough to sRGB, that's fine.
                // TODO: should we maybe do this unconditionally to snap near-sRGB parametrics to sRGB?
                if skcms::trcs_are_approximate_inverse(
                    profile,
                    skcms::srgb_inverse_transfer_function(),
                ) {
                    trfn = named_transfer_fn::SRGB;
                    has_set_trfn = true;
                }
            }
        }
        if !has_set_trfn {
            return None;
        }

        Self::new_rgb(&trfn, &to_xyzd50)
    }

    /// Converts this color space to an skcms ICC profile.
    // Port of: src/core/SkColorSpace.cpp#L298-L331 (chrome/m156)
    #[doc(alias = "toProfile")]
    #[must_use]
    pub fn to_profile(&self) -> IccProfile {
        let mut profile = IccProfile::new();
        // TODO(https://issues.skia.org/issues/420956739): This value should only be
        // set for sRGB-ish transfer functions. All other values are invalid.
        profile.set_transfer_function(&self.0.transfer_fn);
        profile.set_xyzd50(&self.0.to_xyzd50);

        match self.0.transfer_fn.tf_type() {
            TfType::PQ | TfType::PQish => {
                profile.has_cicp = true;
                profile.cicp.transfer_characteristics = named_transfer_fn::CicpId::PQ as u8;
            }
            TfType::HLG | TfType::HLGish => {
                profile.has_cicp = true;
                profile.cicp.transfer_characteristics = named_transfer_fn::CicpId::HLG as u8;
            }
            _ => {}
        }
        if profile.has_cicp {
            profile.cicp.matrix_coefficients = 0;
            profile.cicp.video_full_range_flag = 1;
            if let Some(primaries_id) = named_primaries::get_cicp_from_matrix(&self.0.to_xyzd50) {
                profile.cicp.color_primaries = primaries_id as u8;
            } else {
                profile.cicp.color_primaries = named_primaries::CICP_ID_APPLICATION_DEFINED;
            }
        }
        profile
    }

    /// Creates a [`ColorSpace`] from the bytes of an ICC profile. Returns `None` if `data` is not
    /// a valid ICC profile, or describes an unsupported color space.
    #[must_use]
    pub fn new_icc(data: &[u8]) -> Option<Self> {
        let profile = skcms::parse(data)?;
        Self::make(&profile)
    }

    // Port of: src/core/SkColorSpace.cpp#L206-L225 (chrome/m156)
    fn compute_lazy_dst_fields(&self) -> &LazyDstFields {
        self.0.lazy_dst_fields.get_or_init(|| {
            // Invert 3x3 gamut, defaulting to sRGB if we can't.
            let from_xyzd50 = self.0.to_xyzd50.invert().unwrap_or_else(|| {
                skcms::srgb_profile()
                    .to_xyzd50
                    .invert()
                    .expect("the sRGB gamut is invertible")
            });

            // Invert transfer function, defaulting to sRGB if we can't.
            let inv_transfer_fn = self
                .0
                .transfer_fn
                .invert()
                .unwrap_or(*skcms::srgb_inverse_transfer_function());

            LazyDstFields {
                inv_transfer_fn,
                from_xyzd50,
            }
        })
    }

    /// Returns true if the color space gamma is near enough to be approximated as sRGB.
    // Port of: src/core/SkColorSpace.cpp#L262-L265 (chrome/m156)
    #[doc(alias = "gammaCloseToSRGB")]
    #[must_use]
    pub fn gamma_close_to_srgb(&self) -> bool {
        // Nearly-equal transfer functions were snapped at construction time, so just do an exact test
        self.0.transfer_fn.bit_eq(&named_transfer_fn::SRGB)
    }

    /// Returns true if the color space gamma is linear.
    // Port of: src/core/SkColorSpace.cpp#L267-L270 (chrome/m156)
    #[doc(alias = "gammaIsLinear")]
    #[must_use]
    pub fn gamma_is_linear(&self) -> bool {
        // Nearly-equal transfer functions were snapped at construction time, so just do an exact test
        self.0.transfer_fn.bit_eq(&named_transfer_fn::LINEAR)
    }

    /// Returns the transfer function from this color space if it can be represented as
    /// coefficients to the standard ICC 7-parameter equation. Returns `None` otherwise (eg, PQ,
    /// HLG).
    // Port of: src/core/SkColorSpace.cpp#L227-L233 (chrome/m156)
    #[doc(alias = "isNumericalTransferFn")]
    #[must_use]
    pub fn is_numerical_transfer_fn(&self) -> Option<TransferFunction> {
        // TODO: Change transferFn/invTransferFn to just operate on skcms_TransferFunction (all callers
        // already pass pointers to an skcms struct). Then remove this function, and update the two
        // remaining callers to do the right thing with transferFn and classify.
        let coeffs = self.transfer_fn();
        if coeffs.tf_type() == TfType::SRGBish {
            Some(coeffs)
        } else {
            None
        }
    }

    /// The gamut: the transformation to XYZ D50. (Skia returns true and sets an out-parameter;
    /// it cannot fail.)
    // Port of: src/core/SkColorSpace.cpp#L248-L251 (chrome/m156)
    #[doc(alias = "toXYZD50")]
    #[must_use]
    pub fn to_xyzd50(&self) -> Matrix3x3 {
        self.0.to_xyzd50
    }

    /// Returns a hash of the gamut transformation to XYZ D50. Allows for fast equality checking
    /// of gamuts, at the (very small) risk of collision.
    // Port of: include/core/SkColorSpace.h#L363 (chrome/m156)
    #[doc(alias = "toXYZD50Hash")]
    #[must_use]
    pub fn to_xyzd50_hash(&self) -> XYZD50Hash {
        XYZD50Hash(self.0.to_xyzd50_hash)
    }

    /// Returns a color space with the same gamut as this one, but with a linear gamma.
    ///
    /// # Panics
    /// Never in practice: the linear transfer function is valid.
    // Port of: src/core/SkColorSpace.cpp#L272-L277 (chrome/m156)
    #[doc(alias = "makeLinearGamma")]
    #[must_use]
    pub fn with_linear_gamma(&self) -> Self {
        if self.gamma_is_linear() {
            return self.clone();
        }
        Self::new_rgb(&named_transfer_fn::LINEAR, &self.0.to_xyzd50)
            .expect("the linear transfer function is valid")
    }

    /// Returns a color space with the same gamut as this one, but with the sRGB transfer
    /// function.
    ///
    /// # Panics
    /// Never in practice: the sRGB transfer function is valid.
    // Port of: src/core/SkColorSpace.cpp#L279-L284 (chrome/m156)
    #[doc(alias = "makeSRGBGamma")]
    #[must_use]
    pub fn with_srgb_gamma(&self) -> Self {
        if self.gamma_close_to_srgb() {
            return self.clone();
        }
        Self::new_rgb(&named_transfer_fn::SRGB, &self.0.to_xyzd50)
            .expect("the sRGB transfer function is valid")
    }

    /// Returns a color space with the same transfer function as this one, but with the primary
    /// colors rotated. In other words, this produces a new color space that maps RGB to GBR
    /// (when applied to a source), and maps RGB to BRG (when applied to a destination).
    ///
    /// This is used for testing, to construct color spaces that have severe and testable
    /// behavior.
    // Port of: src/core/SkColorSpace.cpp#L286-L296 (chrome/m156)
    #[doc(alias = "makeColorSpin")]
    #[must_use]
    #[allow(clippy::similar_names)] // mirrors the C++ variable names
    pub fn with_color_spin(&self) -> Self {
        let spin = Matrix3x3 {
            vals: [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        };

        let spun = self.0.to_xyzd50.concat(&spin);

        Self::new_unchecked(self.0.transfer_fn, spun)
    }

    /// Returns true if the color space is sRGB. Returns false otherwise.
    ///
    /// This allows a little bit of tolerance, given that we might see small numerical error in
    /// some cases: converting ICC fixed point to float, converting white point to D50, rounding
    /// decisions on transfer function and matrix.
    ///
    /// This does not consider a 2.2f exponential transfer function to be sRGB. While these
    /// functions are similar (and it is sometimes useful to consider them together), this
    /// function checks for logical equality.
    // Port of: src/core/SkColorSpace.cpp#L258-L260 (chrome/m156)
    #[doc(alias = "isSRGB")]
    #[must_use]
    pub fn is_srgb(&self) -> bool {
        Arc::ptr_eq(&srgb_singleton().0, &self.0)
    }

    /// If `memory` is `None`, returns the size required to serialize. Otherwise, serializes into
    /// `memory` (which must be at least that size) and returns the size.
    // Port of: src/core/SkColorSpace.cpp#L429-L441 (chrome/m156)
    #[doc(alias = "writeToMemory")]
    #[must_use]
    pub fn write_to_memory(&self, memory: Option<&mut [u8]>) -> usize {
        if let Some(memory) = memory {
            memory[..COLOR_SPACE_HEADER_SIZE].copy_from_slice(&[K_CURRENT_VERSION, 0, 0, 0]);
            let mut offset = COLOR_SPACE_HEADER_SIZE;

            for v in self.0.transfer_fn.to_array() {
                memory[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
                offset += 4;
            }

            for v in self.0.to_xyzd50.vals.iter().flatten() {
                memory[offset..offset + 4].copy_from_slice(&v.to_ne_bytes());
                offset += 4;
            }
        }

        COLOR_SPACE_HEADER_SIZE + 16 * std::mem::size_of::<f32>()
    }

    /// Returns a serialized representation of this color space.
    // Port of: src/core/SkColorSpace.cpp#L443-L447 (chrome/m156)
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = vec![0u8; self.write_to_memory(None)];
        let _ = self.write_to_memory(Some(&mut data));
        data
    }

    /// Reads a color space serialized with [`serialize`](Self::serialize). Returns `None` if
    /// `data` is not a valid serialization.
    // Port of: src/core/SkColorSpace.cpp#L449-L472 (chrome/m156)
    #[doc(alias = "Deserialize")]
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < COLOR_SPACE_HEADER_SIZE {
            return None;
        }

        let version = data[0];
        let data = &data[COLOR_SPACE_HEADER_SIZE..];
        if version != K1_VERSION {
            return None;
        }

        if data.len() < 16 * std::mem::size_of::<f32>() {
            return None;
        }

        let mut floats = [0.0f32; 16];
        for (i, v) in floats.iter_mut().enumerate() {
            *v = f32::from_ne_bytes([
                data[4 * i],
                data[4 * i + 1],
                data[4 * i + 2],
                data[4 * i + 3],
            ]);
        }

        let transfer_fn = TransferFunction::new(
            floats[0], floats[1], floats[2], floats[3], floats[4], floats[5], floats[6],
        );
        let to_xyz = Matrix3x3 {
            vals: [
                [floats[7], floats[8], floats[9]],
                [floats[10], floats[11], floats[12]],
                [floats[13], floats[14], floats[15]],
            ],
        };
        Self::new_rgb(&transfer_fn, &to_xyz)
    }

    /// If both are `None`, we return true. If one is `None` and the other is not, we return
    /// false. If both are `Some`, we do a deeper compare.
    // Port of: src/core/SkColorSpace.cpp#L474-L509 (chrome/m156)
    #[doc(alias = "Equals")]
    #[must_use]
    pub fn equals(x: Option<&ColorSpace>, y: Option<&ColorSpace>) -> bool {
        let (x, y) = match (x, y) {
            (None, None) => return true,
            (Some(x), Some(y)) => (x, y),
            _ => return false,
        };
        if Arc::ptr_eq(&x.0, &y.0) {
            return true;
        }

        if x.hash() == y.hash() {
            #[cfg(debug_assertions)]
            {
                // Do these floats function equivalently?
                // This returns true more often than simple float comparison   (NaN vs. NaN) and,
                // also returns true more often than simple bitwise comparison (+0 vs. -0) and,
                // even returns true more often than those two OR'd together   (two different NaNs).
                #[allow(clippy::float_cmp)] // mirrors the C++ (X==Y) || both NaN
                let equiv = |a: f32, b: f32| a == b || (a.is_nan() && b.is_nan());

                for (i, (a, b)) in
                    x.0.transfer_fn
                        .to_array()
                        .iter()
                        .zip(y.0.transfer_fn.to_array().iter())
                        .enumerate()
                {
                    debug_assert!(equiv(*a, *b), "Hash collision at tf[{i}], !equiv({a},{b})");
                }
                for r in 0..3 {
                    for c in 0..3 {
                        let a = x.0.to_xyzd50.vals[r][c];
                        let b = y.0.to_xyzd50.vals[r][c];
                        debug_assert!(
                            equiv(a, b),
                            "Hash collision at toXYZD50[{r}][{c}], !equiv({a},{b})"
                        );
                    }
                }
            }
            return true;
        }
        false
    }

    /// The transfer function.
    // Port of: src/core/SkColorSpace.cpp#L239-L241 (chrome/m156)
    #[doc(alias = "transferFn")]
    #[must_use]
    pub fn transfer_fn(&self) -> TransferFunction {
        self.0.transfer_fn
    }

    /// The inverse of the transfer function (the sRGB one if the transfer function cannot be
    /// inverted).
    // Port of: src/core/SkColorSpace.cpp#L243-L246 (chrome/m156)
    #[doc(alias = "invTransferFn")]
    #[must_use]
    pub fn inv_transfer_fn(&self) -> TransferFunction {
        self.compute_lazy_dst_fields().inv_transfer_fn
    }

    /// The matrix that converts from this color space's gamut to `dst`'s.
    // Port of: src/core/SkColorSpace.cpp#L253-L256 (chrome/m156)
    #[doc(alias = "gamutTransformTo")]
    #[must_use]
    pub fn gamut_transform_to(&self, dst: &ColorSpace) -> Matrix3x3 {
        dst.compute_lazy_dst_fields()
            .from_xyzd50
            .concat(&self.0.to_xyzd50)
    }

    /// A hash of the transfer function.
    // Port of: include/core/SkColorSpace.h#L424 (chrome/m156)
    #[doc(alias = "transferFnHash")]
    #[must_use]
    pub fn transfer_fn_hash(&self) -> u32 {
        self.0.transfer_fn_hash
    }

    /// Returns true if both handles refer to the same color space object (the C++ pointer
    /// comparison `a.get() == b.get()`), as opposed to equal ones (see
    /// [`equals`](Self::equals)).
    #[must_use]
    pub fn ptr_eq(&self, other: &ColorSpace) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// A hash of the transfer function and the gamut.
    // Port of: include/core/SkColorSpace.h#L425 (chrome/m156)
    #[must_use]
    pub fn hash(&self) -> u64 {
        (u64::from(self.0.transfer_fn_hash) << 32) | u64::from(self.0.to_xyzd50_hash)
    }
}
