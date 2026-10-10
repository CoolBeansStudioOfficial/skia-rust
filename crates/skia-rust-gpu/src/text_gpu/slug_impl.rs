// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/SlugImpl.h, src/text/gpu/SlugImpl.cpp

//! [`SlugImpl`]: the GPU implementation of [`Slug`](skia_rust_core::slug::Slug): the sub runs of
//! a text blob drawn with a paint, kept so they can be drawn again with a different matrix and
//! clip.
//!
//! Not ported: `doFlatten` and `MakeFromBuffer` (serialization needs the remote glyph cache,
//! T23).

use core::any::Any;

use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::slug::SlugBase;

use crate::text_gpu::sub_run_container::{
    StrikeDeviceInfo, SubRunContainer, SubRunCreationBehavior,
};

/// The slug of a blob (`sktext::gpu::SlugImpl`).
// Port of: src/text/gpu/SlugImpl.h#L33-L67 (chrome/m156)
#[doc(alias = "sktext::gpu::SlugImpl")]
#[derive(Debug)]
pub struct SlugImpl {
    /// `fSubRuns`.
    sub_runs: SubRunContainer,
    /// `fSourceBounds`.
    source_bounds: Rect,
    /// `fOrigin`.
    origin: Point,
}

impl SlugImpl {
    /// `SlugImpl(alloc, subRuns, sourceBounds, origin)`.
    // Port of: src/text/gpu/SlugImpl.cpp#L28-L36 (chrome/m156)
    #[must_use]
    pub fn new(sub_runs: SubRunContainer, source_bounds: Rect, origin: Point) -> Self {
        Self {
            sub_runs,
            source_bounds,
            origin,
        }
    }

    /// `position_matrix(drawMatrix, drawOrigin)`: `drawMatrix * translate(drawOrigin)`.
    // Port of: src/text/gpu/SlugImpl.cpp#L68-L71 (chrome/m156)
    fn position_matrix(draw_matrix: &Matrix, draw_origin: Point) -> Matrix {
        let mut position_matrix = draw_matrix.clone();
        position_matrix.pre_translate(draw_origin);
        position_matrix
    }

    /// `Make(viewMatrix, glyphRunList, paint, strikeDeviceInfo, strikeCache)`. Returns `None`
    /// if there is nothing to draw here. This is particularly a problem with `RSXform` blobs
    /// where a single space becomes a run with no glyphs.
    // Port of: src/text/gpu/SlugImpl.cpp#L73-L99 (chrome/m156)
    #[must_use]
    pub fn make(
        view_matrix: &Matrix,
        glyph_run_list: &GlyphRunList<'_>,
        paint: &Paint,
        strike_device_info: &StrikeDeviceInfo,
    ) -> Option<SlugImpl> {
        let position_matrix = Self::position_matrix(view_matrix, glyph_run_list.origin());

        let sub_runs = SubRunContainer::make(
            glyph_run_list,
            &position_matrix,
            paint,
            strike_device_info,
            SubRunCreationBehavior::AddSubRuns,
        );

        if sub_runs.is_empty() {
            return None;
        }

        Some(SlugImpl::new(
            sub_runs,
            glyph_run_list.source_bounds(),
            glyph_run_list.origin(),
        ))
    }

    /// `subRuns()`.
    // Port of: src/text/gpu/SlugImpl.h#L51 (chrome/m156)
    #[must_use]
    pub fn sub_runs(&self) -> &SubRunContainer {
        &self.sub_runs
    }

    /// `origin()`.
    // Port of: src/text/gpu/SlugImpl.h#L52 (chrome/m156)
    #[must_use]
    pub fn origin(&self) -> Point {
        self.origin
    }
}

impl SlugBase for SlugImpl {
    // Port of: src/text/gpu/SlugImpl.h#L45 (chrome/m156)
    fn source_bounds(&self) -> Rect {
        self.source_bounds
    }

    // Port of: src/text/gpu/SlugImpl.h#L46-L48 (chrome/m156)
    fn source_bounds_with_origin(&self) -> Rect {
        self.source_bounds.with_offset(self.origin)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
