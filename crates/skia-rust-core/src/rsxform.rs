// Copyright 2018 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkRSXform.h (chrome/m156), the fields and `Make` only.

//! `SkRSXform`: a rotation-scale transform plus translation, used by `drawGlyphs` with `RSXform`
//! positioning.

use crate::scalar::scalar;

/// A rotation-scale transform: `(s_cos, s_sin)` is the scale times the cosine and sine of the
/// rotation, and `(tx, ty)` the translation.
// Port of: include/core/SkRSXform.h#L23-L45 (chrome/m156)
#[doc(alias = "SkRSXform")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RSXform {
    /// `fSCos`.
    pub s_cos: scalar,
    /// `fSSin`.
    pub s_sin: scalar,
    /// `fTx`.
    pub tx: scalar,
    /// `fTy`.
    pub ty: scalar,
}

impl RSXform {
    /// `SkRSXform::Make(scos, ssin, tx, ty)`.
    // Port of: include/core/SkRSXform.h#L24-L27 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub const fn new(s_cos: scalar, s_sin: scalar, tx: scalar, ty: scalar) -> Self {
        Self {
            s_cos,
            s_sin,
            tx,
            ty,
        }
    }
}
