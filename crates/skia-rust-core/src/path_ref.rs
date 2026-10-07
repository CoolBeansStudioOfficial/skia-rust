// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkPathRef.h

//! Shape-identification records for paths (`SkPathRef.h`).
//!
//! In m156 `SkPathRef.h` only holds these small structs; the path storage itself is
//! [`PathData`](crate::path_data::PathData).

use crate::path_types::PathDirection;
use crate::rect::Rect;
use crate::rrect::RRect;

/// Returned when a path is recognized as a rectangle.
// Port of: include/private/SkPathRef.h#L36-L40 (chrome/m156)
#[doc(alias = "SkPathRectInfo")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct PathRectInfo {
    pub rect: Rect,
    pub direction: PathDirection,
    pub start_index: u8,
}

/// Returned when a path is recognized as an oval.
// Port of: include/private/SkPathRef.h#L42-L46 (chrome/m156)
#[doc(alias = "SkPathOvalInfo")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct PathOvalInfo {
    pub bounds: Rect,
    pub direction: PathDirection,
    pub start_index: u8,
}

/// Returned when a path is recognized as a round rectangle.
// Port of: include/private/SkPathRef.h#L48-L52 (chrome/m156)
#[doc(alias = "SkPathRRectInfo")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct PathRRectInfo {
    pub rrect: RRect,
    pub direction: PathDirection,
    pub start_index: u8,
}

/// The high-level shape a path was built from.
// Port of: include/private/SkPathRef.h#L79-L83 (chrome/m156)
#[doc(alias = "SkPathIsAType")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[repr(u8)]
pub enum PathIsAType {
    #[default]
    General,
    Oval,
    RRect,
}

/// The data stored with a [`PathIsAType`].
// Port of: include/private/SkPathRef.h#L85-L88 (chrome/m156)
#[doc(alias = "SkPathIsAData")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct PathIsAData {
    pub start_index: u8,
    pub direction: PathDirection,
}
