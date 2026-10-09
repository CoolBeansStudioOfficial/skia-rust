// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/RendererProvider.h, src/gpu/graphite/RendererProvider.cpp
//
// PARTIAL. This is the subset of `RendererProvider` whose render steps are ported: the
// single-step renderers for PerEdgeAAQuad, CircularArc and CoverBounds[NonAAFill], and the cover
// steps the tessellating renderers will share. Still missing, each with its step's port:
// - the path renderer strategy (`IsSupported`, the strategy choice in the constructor) needs
//   `Caps` (`requestedPathRendererStrategy`, `avoidMSAA`, `minPathSizeForMSAA`), which is G10;
// - `fAnalyticRRect` needs `AnalyticRRectRenderStep` (G7a, not yet ported);
// - `fCoverageMask` needs `CoverageMaskRenderStep` (needs `CoverageMaskShape`, G2);
// - `fVertices[*]` needs `VerticesRenderStep`, and `fMesh` needs `MeshRenderStep` (SkMesh,
//   not ported);
// - the tessellation renderers, the bitmap and SDF text renderers, the blur renderers and the
//   sparse-strip renderers need G7b, G7c and G17.

use std::sync::Arc;

use crate::graphite::buffer_manager::StaticBufferManager;
use crate::graphite::draw_types::DrawTypeFlags;
use crate::graphite::render::circular_arc_render_step::CircularArcRenderStep;
use crate::graphite::render::common_depth_stencil_settings::{
    DIRECT_DEPTH_LESS_PASS, INVERSE_COVER_PASS, REGULAR_COVER_PASS,
};
use crate::graphite::render::cover_bounds_render_step::CoverBoundsRenderStep;
use crate::graphite::render::per_edge_aa_quad_render_step::PerEdgeAAQuadRenderStep;
use crate::graphite::render_step::{RenderStep, RenderStepID};
use crate::graphite::renderer::Renderer;
use crate::graphite::resource_types::Layout;

/// The renderers of the steps ported so far, and the cover steps the tessellating renderers
/// share (`RendererProvider`, partial).
// Port of: src/gpu/graphite/RendererProvider.h (the members for the ported steps)
#[doc(alias = "skgpu::graphite::RendererProvider")]
#[derive(Debug)]
pub struct RendererProvider {
    /// `fPerEdgeAAQuad`.
    per_edge_aa_quad: Renderer,
    /// `fNonAABoundsFill`.
    non_aa_bounds_fill: Renderer,
    /// `fCircularArc`.
    circular_arc: Renderer,
    /// The regular cover step of the stencil-then-cover renderers (`coverFill`).
    cover_fill: Arc<dyn RenderStep>,
    /// The inverse cover step of the stencil-then-cover renderers (`coverInverse`).
    cover_inverse: Arc<dyn RenderStep>,
}

impl RendererProvider {
    /// `RendererProvider(caps, bufferManager)` for the steps ported so far.
    // Port of: src/gpu/graphite/RendererProvider.cpp#L87 (chrome/m156), the ported initializers
    #[must_use]
    pub fn new(layout: Layout, buffer_manager: &mut StaticBufferManager) -> Self {
        let per_edge_aa_quad = single_step(
            Arc::new(PerEdgeAAQuadRenderStep::new(layout, buffer_manager)),
            DrawTypeFlags::PER_EDGE_AA_QUAD,
        );
        let non_aa_bounds_fill = single_step(
            Arc::new(CoverBoundsRenderStep::new(
                layout,
                RenderStepID::CoverBounds_NonAAFill,
                DIRECT_DEPTH_LESS_PASS,
            )),
            DrawTypeFlags::NON_AA_FILL_RECT,
        );
        let circular_arc = single_step(
            Arc::new(CircularArcRenderStep::new(layout, buffer_manager)),
            DrawTypeFlags::CIRCULAR_ARC,
        );
        Self {
            per_edge_aa_quad,
            non_aa_bounds_fill,
            circular_arc,
            cover_fill: Arc::new(CoverBoundsRenderStep::new(
                layout,
                RenderStepID::CoverBounds_RegularCover,
                REGULAR_COVER_PASS,
            )),
            cover_inverse: Arc::new(CoverBoundsRenderStep::new(
                layout,
                RenderStepID::CoverBounds_InverseCover,
                INVERSE_COVER_PASS,
            )),
        }
    }

    /// `fPerEdgeAAQuad`.
    // Port of: src/gpu/graphite/RendererProvider.h (fPerEdgeAAQuad)
    #[must_use]
    pub const fn per_edge_aa_quad(&self) -> &Renderer {
        &self.per_edge_aa_quad
    }

    /// `fNonAABoundsFill`.
    // Port of: src/gpu/graphite/RendererProvider.h (fNonAABoundsFill)
    #[must_use]
    pub const fn non_aa_bounds_fill(&self) -> &Renderer {
        &self.non_aa_bounds_fill
    }

    /// `fCircularArc`.
    // Port of: src/gpu/graphite/RendererProvider.h (fCircularArc)
    #[must_use]
    pub const fn circular_arc(&self) -> &Renderer {
        &self.circular_arc
    }

    /// `coverFill`: the regular cover step, shared by the stencil-then-cover renderers.
    // Port of: src/gpu/graphite/RendererProvider.cpp#L205-L206 (chrome/m156)
    #[must_use]
    pub fn cover_fill(&self) -> &Arc<dyn RenderStep> {
        &self.cover_fill
    }

    /// `coverInverse`: the inverse cover step, shared by the stencil-then-cover renderers.
    // Port of: src/gpu/graphite/RendererProvider.cpp#L207-L208 (chrome/m156)
    #[must_use]
    pub fn cover_inverse(&self) -> &Arc<dyn RenderStep> {
        &self.cover_inverse
    }
}

/// `initFromStep`: a renderer made of one step, named `SingleStep[<step name>]`. Single-step
/// renderers do not share their step.
// Port of: src/gpu/graphite/RendererProvider.cpp#L116-L125 (chrome/m156)
fn single_step(step: Arc<dyn RenderStep>, draw_types: DrawTypeFlags) -> Renderer {
    let name = format!("SingleStep[{}]", step.name());
    Renderer::new(&name, draw_types, vec![step])
}
