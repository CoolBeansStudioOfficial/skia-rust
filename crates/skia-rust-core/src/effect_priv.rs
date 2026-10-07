// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkEffectPriv.h

//! `SkStageRec`: what effects (shaders, color filters, blenders) are given to append their
//! raster pipeline stages.

use crate::arena_alloc::ArenaAlloc;
use crate::color::Color4f;
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::raster_pipeline::RasterPipeline;
use crate::rect::Rect;
use crate::surface_props::SurfaceProps;

/// Passed to effects that will add stages to a raster pipeline (`SkStageRec`).
///
/// Skia passes it as `const SkStageRec&` holding a non-const `SkRasterPipeline*`; here the
/// effects take `&mut StageRec`, which holds the pipeline mutably. Contexts the effects
/// allocate go in `alloc`, which outlives the pipeline (`'a`).
///
// Port of: src/core/SkEffectPriv.h#L20-L32 (chrome/m156)
#[doc(alias = "SkStageRec")]
#[derive(Debug)]
pub struct StageRec<'r, 'a> {
    /// `fPipeline`.
    pub pipeline: &'r mut RasterPipeline<'a>,
    /// `fAlloc`.
    pub alloc: &'a ArenaAlloc,
    /// `fDstColorType`.
    pub dst_color_type: ColorType,
    /// `fDstCS` (may be `None`).
    pub dst_cs: Option<&'r ColorSpace>,
    /// `fPaintColor`.
    pub paint_color: Color4f,
    /// `fSurfaceProps`: the properties of the surface being drawn to (D6 plumbs them from the
    /// device through the blitter chooser).
    pub surface_props: SurfaceProps,
    /// `fDstBounds`: the device-space bounding box of the geometry being drawn. An empty value
    /// can be used when it is expensive to compute, in which case a heuristic will be used if
    /// necessary.
    pub dst_bounds: Rect,
}
