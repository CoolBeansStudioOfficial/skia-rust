// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPaint.h, src/core/SkPaintDefaults.h

//! The stroke enums of `SkPaint` (`Cap`, `Join`, `Style`) and the paint defaults the stroker
//! uses.
//!
//! skia-rust: `SkPaint` itself is not ported yet; only the pieces the stroker (`SkStroke`,
//! `SkStrokeRec`, `SkStrokerPriv`) depends on live here, at the `skia_safe::paint` paths.

use crate::scalar::scalar;

/// The default miter limit (`SkPaintDefaults_MiterLimit`).
// Port of: src/core/SkPaintDefaults.h#L27-L29 (chrome/m156)
#[doc(alias = "SkPaintDefaults_MiterLimit")]
pub const DEFAULT_MITER_LIMIT: scalar = 4.0;

/// How the ends of an open contour are drawn (`SkPaint::Cap`).
// Port of: include/core/SkPaint.h#L336-L342 (chrome/m156)
#[doc(alias = "SkPaint::Cap")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Cap {
    /// No stroke extension (`kButt_Cap`).
    #[default]
    Butt = 0,
    /// Adds a circle (`kRound_Cap`).
    Round = 1,
    /// Adds a square (`kSquare_Cap`).
    Square = 2,
}

impl Cap {
    /// The number of caps (`SkPaint::kCapCount`).
    #[doc(alias = "kCapCount")]
    pub const COUNT: usize = 3;
    /// The largest cap (`SkPaint::kLast_Cap`).
    #[doc(alias = "kLast_Cap")]
    pub const LAST: Cap = Cap::Square;
    /// `kDefault_Cap`.
    #[doc(alias = "kDefault_Cap")]
    pub const DEFAULT: Cap = Cap::Butt;
}

/// How corners between segments are drawn (`SkPaint::Join`).
// Port of: include/core/SkPaint.h#L361-L367 (chrome/m156)
#[doc(alias = "SkPaint::Join")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Join {
    /// Extends to the miter limit (`kMiter_Join`).
    #[default]
    Miter = 0,
    /// Adds a circle (`kRound_Join`).
    Round = 1,
    /// Connects the outside edges (`kBevel_Join`).
    Bevel = 2,
}

impl Join {
    /// The number of joins (`SkPaint::kJoinCount`).
    #[doc(alias = "kJoinCount")]
    pub const COUNT: usize = 3;
    /// The largest join (`SkPaint::kLast_Join`).
    #[doc(alias = "kLast_Join")]
    pub const LAST: Join = Join::Bevel;
    /// `kDefault_Join`.
    #[doc(alias = "kDefault_Join")]
    pub const DEFAULT: Join = Join::Miter;
}

/// Whether geometry is filled, stroked or both (`SkPaint::Style`).
// Port of: include/core/SkPaint.h#L168-L173 (chrome/m156)
#[doc(alias = "SkPaint::Style")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Style {
    /// Fills the geometry (`kFill_Style`).
    #[default]
    Fill = 0,
    /// Strokes the geometry (`kStroke_Style`).
    Stroke = 1,
    /// Fills and strokes the geometry (`kStrokeAndFill_Style`).
    StrokeAndFill = 2,
}
