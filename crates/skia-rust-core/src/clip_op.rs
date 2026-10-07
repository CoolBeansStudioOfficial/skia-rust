// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkClipOp.h

//! [`ClipOp`]: how a clip combines with the existing clip.

use crate::region::Op;

/// The two ways a clip can be combined with the current clip (`SkClipOp`). They are the first two
/// values of [`Op`] (`SkRegion::Op`), which Skia relies on when it casts one to the other.
// Port of: include/core/SkClipOp.h#L13-L17 (chrome/m156)
#[doc(alias = "SkClipOp")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ClipOp {
    /// Subtracts the new clip from the current clip.
    #[doc(alias = "kDifference")]
    Difference = 0,
    /// Intersects the new clip with the current clip.
    #[doc(alias = "kIntersect")]
    Intersect = 1,
}

impl ClipOp {
    /// `kMax_EnumValue`.
    #[doc(alias = "kMax_EnumValue")]
    pub const MAX_ENUM_VALUE: ClipOp = ClipOp::Intersect;
}

/// `(SkRegion::Op) op`: the region operation with the same discriminant.
impl From<ClipOp> for Op {
    fn from(op: ClipOp) -> Op {
        match op {
            ClipOp::Difference => Op::Difference,
            ClipOp::Intersect => Op::Intersect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminants_match_region_ops() {
        assert_eq!(ClipOp::Difference as u32, Op::Difference as u32);
        assert_eq!(ClipOp::Intersect as u32, Op::Intersect as u32);
        assert_eq!(Op::from(ClipOp::Difference), Op::Difference);
        assert_eq!(Op::from(ClipOp::Intersect), Op::Intersect);
    }
}
