// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/pathops/SkPathOps.h (SkPathOp)

//! The boolean operators of `PathOps` (`SkPathOp`).

/// `SkPathOp`: the boolean operation to perform. The discriminants are Skia's.
// Port of: include/pathops/SkPathOps.h (SkPathOp) (chrome/m156)
#[doc(alias = "SkPathOp")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathOp {
    /// `kDifference_SkPathOp`: subtract the second path from the first.
    Difference = 0,
    /// `kIntersect_SkPathOp`: the area common to both paths.
    Intersect = 1,
    /// `kUnion_SkPathOp`: the area covered by either path.
    Union = 2,
    /// `kXOR_SkPathOp`: the area covered by exactly one path.
    Xor = 3,
    /// `kReverseDifference_SkPathOp`: subtract the first path from the second.
    ReverseDifference = 4,
}
