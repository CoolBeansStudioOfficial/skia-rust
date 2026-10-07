// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPathRaw.h, src/core/SkPathRaw.cpp

//! A non-owning, immutable view of path geometry (`SkPathRaw.h`).

use crate::path_enums::PathConvexity;
use crate::path_iter::PathIter;
use crate::path_priv;
use crate::path_types::{PathFillType, PathVerb};
use crate::point::Point;
use crate::rect::Rect;
use crate::scalar::scalar;

/// A non-owning, immutable view of the path geometry.
///
/// It allows us to have stack-allocated paths, see [`path_raw_shapes`](crate::path_raw_shapes).
// Port of: src/core/SkPathRaw.h#L19-L60 (chrome/m156)
#[doc(alias = "SkPathRaw")]
#[derive(Copy, Clone, Debug)]
pub struct PathRaw<'a> {
    pub points: &'a [Point],
    pub verbs: &'a [PathVerb],
    pub conics: &'a [scalar],
    pub bounds: Rect,
    pub fill_type: PathFillType,
    pub convexity: PathConvexity,
    /// See `Path::segment_masks`.
    pub segment_mask: u8,
}

impl<'a> PathRaw<'a> {
    #[must_use]
    pub fn points(&self) -> &'a [Point] {
        self.points
    }

    #[must_use]
    pub fn verbs(&self) -> &'a [PathVerb] {
        self.verbs
    }

    #[must_use]
    pub fn conics(&self) -> &'a [scalar] {
        self.conics
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    #[doc(alias = "fillType")]
    #[must_use]
    pub fn fill_type(&self) -> PathFillType {
        self.fill_type
    }

    #[must_use]
    pub fn convexity(&self) -> PathConvexity {
        self.convexity
    }

    #[doc(alias = "segmentMasks")]
    #[must_use]
    pub fn segment_masks(&self) -> u32 {
        u32::from(self.segment_mask)
    }

    #[doc(alias = "empty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    #[doc(alias = "isInverseFillType")]
    #[must_use]
    pub fn is_inverse_fill_type(&self) -> bool {
        self.fill_type.is_inverse()
    }

    #[doc(alias = "isKnownToBeConvex")]
    #[must_use]
    pub fn is_known_to_be_convex(&self) -> bool {
        self.convexity.is_convex()
    }

    /// The rectangle, if this path is one.
    // Port of: src/core/SkPathRaw.cpp#L33-L38 (chrome/m156)
    #[doc(alias = "isRect")]
    #[must_use]
    pub fn is_rect(&self) -> Option<Rect> {
        path_priv::is_rect_contour(self.points, self.verbs, u32::from(self.segment_mask), false)
            .map(|rc| rc.rect)
    }

    /// Iterates the verbs.
    #[must_use]
    #[allow(clippy::iter_without_into_iter)] // mirrors the C++ / skia-safe `iter()`
    pub fn iter(&self) -> PathIter<'a> {
        PathIter::new(self.points, self.verbs, self.conics)
    }

    /// An empty view.
    // Port of: src/core/SkPathRaw.h#L53-L57 (chrome/m156)
    #[doc(alias = "Empty")]
    #[must_use]
    pub fn empty(ft: PathFillType) -> PathRaw<'static> {
        PathRaw {
            points: &[],
            verbs: &[],
            conics: &[],
            bounds: Rect::new_empty(),
            fill_type: ft,
            convexity: PathConvexity::ConvexDegenerate,
            segment_mask: 0,
        }
    }
}
