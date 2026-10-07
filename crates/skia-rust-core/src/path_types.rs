// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathTypes.h

//! Enums shared by paths and path builders (`SkPathTypes.h`).

use bitflags::bitflags;

/// How "inside" is computed when a path is filled.
// Port of: include/core/SkPathTypes.h#L15-L26 (chrome/m156)
#[doc(alias = "SkPathFillType")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum PathFillType {
    /// Specifies that "inside" is computed by a non-zero sum of signed edge crossings.
    #[default]
    Winding = 0,
    /// Specifies that "inside" is computed by an odd number of edge crossings.
    EvenOdd = 1,
    /// Same as `Winding`, but draws outside of the path, rather than inside.
    InverseWinding = 2,
    /// Same as `EvenOdd`, but draws outside of the path, rather than inside.
    InverseEvenOdd = 3,
}

impl PathFillType {
    /// `SkPathFillType::kDefault`.
    pub const DEFAULT: Self = Self::Winding;

    /// Converts the low two bits of `v` (the C++ `static_cast<SkPathFillType>`).
    #[must_use]
    pub const fn from_bits(v: u32) -> Self {
        match v & 3 {
            0 => Self::Winding,
            1 => Self::EvenOdd,
            2 => Self::InverseWinding,
            _ => Self::InverseEvenOdd,
        }
    }

    /// True for `EvenOdd` and `InverseEvenOdd`.
    // Port of: include/core/SkPathTypes.h#L28-L30 (chrome/m156)
    #[doc(alias = "SkPathFillType_IsEvenOdd")]
    #[must_use]
    pub const fn is_even_odd(self) -> bool {
        (self as u8 & 1) != 0
    }

    /// True for `InverseWinding` and `InverseEvenOdd`.
    // Port of: include/core/SkPathTypes.h#L32-L34 (chrome/m156)
    #[doc(alias = "SkPathFillType_IsInverse")]
    #[must_use]
    pub const fn is_inverse(self) -> bool {
        (self as u8 & 2) != 0
    }

    /// Toggles between the inverse and non-inverse variants.
    // Port of: include/core/SkPathTypes.h#L36-L38 (chrome/m156)
    #[doc(alias = "SkPathFillType_ToggleInverse")]
    #[must_use]
    pub const fn toggle_inverse(self) -> Self {
        Self::from_bits(self as u32 ^ 2)
    }

    /// The non-inverse variant.
    // Port of: include/core/SkPathTypes.h#L40-L42 (chrome/m156)
    #[doc(alias = "SkPathFillType_ConvertToNonInverse")]
    #[must_use]
    pub const fn to_non_inverse(self) -> Self {
        Self::from_bits(self as u32 & 1)
    }
}

/// Direction for adding closed contours.
// Port of: include/core/SkPathTypes.h#L44-L51 (chrome/m156)
#[doc(alias = "SkPathDirection")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum PathDirection {
    /// Clockwise direction for adding closed contours.
    #[default]
    CW = 0,
    /// Counter-clockwise direction for adding closed contours.
    CCW = 1,
}

impl PathDirection {
    /// `SkPathDirection::kDefault`.
    pub const DEFAULT: Self = Self::CW;
}

bitflags! {
    /// One bit per kind of segment in a path.
    // Port of: include/core/SkPathTypes.h#L53-L58 (chrome/m156)
    #[doc(alias = "SkPathSegmentMask")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct PathSegmentMask: u32 {
        const LINE = 1 << 0;
        const QUAD = 1 << 1;
        const CONIC = 1 << 2;
        const CUBIC = 1 << 3;
    }
}

/// The verbs stored in a path.
// Port of: include/core/SkPathTypes.h#L60-L69 (chrome/m156)
#[doc(alias = "SkPathVerb")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum PathVerb {
    /// `SkPath::RawIter` returns 1 point.
    Move = 0,
    /// `SkPath::RawIter` returns 2 points.
    Line = 1,
    /// `SkPath::RawIter` returns 3 points.
    Quad = 2,
    /// `SkPath::RawIter` returns 3 points + 1 weight.
    Conic = 3,
    /// `SkPath::RawIter` returns 4 points.
    Cubic = 4,
    /// `SkPath::RawIter` returns 0 points.
    Close = 5,
}

impl PathVerb {
    /// `SkPathVerb::kLast_Verb`.
    pub const LAST: Self = Self::Close;

    /// The verb for a raw byte, or `None` if it is out of range.
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Move,
            1 => Self::Line,
            2 => Self::Quad,
            3 => Self::Conic,
            4 => Self::Cubic,
            5 => Self::Close,
            _ => return None,
        })
    }
}
