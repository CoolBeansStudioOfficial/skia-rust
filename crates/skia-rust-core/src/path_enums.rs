// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPathEnums.h

//! Private enums related to paths (`SkPathEnums.h`).

use crate::path_types::PathDirection;

/// Convexity of a path, with the winding direction when it is known.
// Port of: src/core/SkPathEnums.h#L17-L25 (chrome/m156)
#[doc(alias = "SkPathConvexity")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum PathConvexity {
    ConvexCW = 0,
    ConvexCCW = 1,
    /// Known to not have a determinable direction, but convex.
    ConvexDegenerate = 2,
    Concave = 3,
    Unknown = 4,
}

impl PathConvexity {
    /// Converts the value stored in an atomic back to the enum.
    #[must_use]
    pub(crate) const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::ConvexCW,
            1 => Self::ConvexCCW,
            2 => Self::ConvexDegenerate,
            3 => Self::Concave,
            _ => Self::Unknown,
        }
    }

    /// `SkPathConvexity_IsConvex`.
    // Port of: src/core/SkPathEnums.h#L32-L36 (chrome/m156)
    #[doc(alias = "SkPathConvexity_IsConvex")]
    #[must_use]
    pub const fn is_convex(self) -> bool {
        matches!(
            self,
            Self::ConvexCW | Self::ConvexCCW | Self::ConvexDegenerate
        )
    }

    /// `SkPathConvexity_OppositeConvexDirection`.
    // Port of: src/core/SkPathEnums.h#L38-L46 (chrome/m156)
    #[doc(alias = "SkPathConvexity_OppositeConvexDirection")]
    #[must_use]
    pub fn opposite_convex_direction(self) -> Self {
        debug_assert!(self.is_convex());
        match self {
            Self::ConvexCW => Self::ConvexCCW,
            Self::ConvexCCW => Self::ConvexCW,
            other => other,
        }
    }

    /// `SkPathConvexity_ToDirection`.
    // Port of: src/core/SkPathEnums.h#L70-L78 (chrome/m156)
    #[doc(alias = "SkPathConvexity_ToDirection")]
    #[must_use]
    pub const fn to_direction(self) -> Option<PathDirection> {
        match self {
            Self::ConvexCW => Some(PathDirection::CW),
            Self::ConvexCCW => Some(PathDirection::CCW),
            _ => None,
        }
    }

    /// `SkPathConvexity_ToFirstDirection`.
    // Port of: src/core/SkPathEnums.h#L80-L88 (chrome/m156)
    #[doc(alias = "SkPathConvexity_ToFirstDirection")]
    #[must_use]
    pub const fn to_first_direction(self) -> PathFirstDirection {
        match self {
            Self::ConvexCW => PathFirstDirection::CW,
            Self::ConvexCCW => PathFirstDirection::CCW,
            _ => PathFirstDirection::Unknown,
        }
    }
}

/// Whether a raw view should compute (and cache) an unknown convexity.
// Port of: src/core/SkPathEnums.h#L27-L30 (chrome/m156)
#[doc(alias = "SkResolveConvexity")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ResolveConvexity {
    No,
    Yes,
}

/// Winding direction of the first (outer-most) contour.
// Port of: src/core/SkPathEnums.h#L48-L52 (chrome/m156)
#[doc(alias = "SkPathFirstDirection")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum PathFirstDirection {
    /// `== PathDirection::CW`
    CW = 0,
    /// `== PathDirection::CCW`
    CCW = 1,
    Unknown = 2,
}

impl PathFirstDirection {
    /// `SkPathFirstDirection_ToConvexity`.
    // Port of: src/core/SkPathEnums.h#L62-L68 (chrome/m156)
    #[doc(alias = "SkPathFirstDirection_ToConvexity")]
    #[must_use]
    pub const fn to_convexity(self) -> PathConvexity {
        match self {
            Self::CW => PathConvexity::ConvexCW,
            Self::CCW => PathConvexity::ConvexCCW,
            Self::Unknown => PathConvexity::ConvexDegenerate,
        }
    }
}

/// `SkPathDirection_ToConvexity`.
// Port of: src/core/SkPathEnums.h#L54-L60 (chrome/m156)
#[doc(alias = "SkPathDirection_ToConvexity")]
#[must_use]
pub const fn direction_to_convexity(dir: PathDirection) -> PathConvexity {
    match dir {
        PathDirection::CW => PathConvexity::ConvexCW,
        PathDirection::CCW => PathConvexity::ConvexCCW,
    }
}

/// `SkPathDirectionToFirst`.
// Port of: src/core/SkPathEnums.h#L90-L93 (chrome/m156)
#[doc(alias = "SkPathDirectionToFirst")]
#[must_use]
pub const fn direction_to_first(dir: PathDirection) -> PathFirstDirection {
    match dir {
        PathDirection::CW => PathFirstDirection::CW,
        PathDirection::CCW => PathFirstDirection::CCW,
    }
}
