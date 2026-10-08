// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkBlurTypes.h

//! `SkBlurStyle`: how a blur mask filter combines the blurred and the original coverage.

/// How a blur is combined with the original mask.
// Port of: include/core/SkBlurTypes.h#L11-L18 (chrome/m156)
#[doc(alias = "SkBlurStyle")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
#[repr(i32)]
pub enum BlurStyle {
    /// Fuzzy inside and outside.
    #[doc(alias = "kNormal_SkBlurStyle")]
    Normal,
    /// Solid inside, fuzzy outside.
    #[doc(alias = "kSolid_SkBlurStyle")]
    Solid,
    /// Nothing inside, fuzzy outside.
    #[doc(alias = "kOuter_SkBlurStyle")]
    Outer,
    /// Fuzzy inside, nothing outside.
    #[doc(alias = "kInner_SkBlurStyle")]
    Inner,
}

impl BlurStyle {
    /// The last enumerator (`kLastEnum_SkBlurStyle`).
    #[doc(alias = "kLastEnum_SkBlurStyle")]
    pub const LAST_ENUM: BlurStyle = BlurStyle::Inner;

    /// The style with discriminant `value`, if there is one.
    #[must_use]
    pub fn from_i32(value: i32) -> Option<BlurStyle> {
        match value {
            0 => Some(BlurStyle::Normal),
            1 => Some(BlurStyle::Solid),
            2 => Some(BlurStyle::Outer),
            3 => Some(BlurStyle::Inner),
            _ => None,
        }
    }
}
