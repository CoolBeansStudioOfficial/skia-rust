// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/RendererProvider.h, src/gpu/graphite/RendererProvider.cpp
//
// PARTIAL. This is the subset of `RendererProvider` whose render steps are ported: the
// single-step renderers for PerEdgeAAQuad, CircularArc, CoverBounds[NonAAFill], the text steps,
// the analytic blurs (AnalyticBlur, AnalyticRRectBlur) and the tessellating renderers (convex
// wedges, stencil curves and wedges with the middle-out fan, and strokes), and the coverage mask
// renderer. The path renderer strategy is chosen from `Caps` as in the C++ constructor. Still
// missing, each with its step's port:
// - the Vello compute strategies (`kComputeAnalyticAA`, `kComputeMSAA16`, `kComputeMSAA8`) are
//   never supported: Skia builds them only with `SK_ENABLE_VELLO_SHADERS`, which is off here;
// - the sparse-strip strategy (`kCPUSparseStripsMSAA8`, G17) is never supported, and the
//   sparse-strip renderers (`EndCap`, `WideTile`) need G17.

use std::sync::Arc;

use skia_rust_core::path_types::PathFillType;

use crate::gpu::mask_format::MaskFormat;
use crate::graphite::buffer_manager::StaticBufferManager;
use crate::graphite::caps::Caps;
use crate::graphite::draw_types::DrawTypeFlags;
use crate::graphite::render::analytic_blur_render_step::AnalyticBlurRenderStep;
use crate::graphite::render::analytic_rrect_blur_render_step::AnalyticRRectBlurRenderStep;
use crate::graphite::render::analytic_rrect_render_step::AnalyticRRectRenderStep;
use crate::graphite::render::bitmap_text_render_step::BitmapTextRenderStep;
use crate::graphite::render::circular_arc_render_step::CircularArcRenderStep;
use crate::graphite::render::common_depth_stencil_settings::{
    DIRECT_DEPTH_LESS_PASS, EVEN_ODD_STENCIL_PASS, INVERSE_COVER_PASS, REGULAR_COVER_PASS,
    WINDING_STENCIL_PASS,
};
use crate::graphite::render::cover_bounds_render_step::CoverBoundsRenderStep;
use crate::graphite::render::coverage_mask_render_step::CoverageMaskRenderStep;
use crate::graphite::render::mesh_render_step::MeshRenderStep;
use crate::graphite::render::middle_out_fan_render_step::MiddleOutFanRenderStep;
use crate::graphite::render::per_edge_aa_quad_render_step::PerEdgeAAQuadRenderStep;
use crate::graphite::render::sdf_text_lcd_render_step::SDFTextLCDRenderStep;
use crate::graphite::render::sdf_text_render_step::SDFTextRenderStep;
use crate::graphite::render::tessellate_curves_render_step::TessellateCurvesRenderStep;
use crate::graphite::render::tessellate_strokes_render_step::TessellateStrokesRenderStep;
use crate::graphite::render::tessellate_wedges_render_step::TessellateWedgesRenderStep;
use crate::graphite::render::vertices_render_step::VerticesRenderStep;
use crate::graphite::render_step::{NUM_RENDER_STEPS, RenderStep, RenderStepID};
use crate::graphite::renderer::Renderer;
use crate::graphite::resource_types::Layout;

/// `PathRendererStrategy`: how paths are rendered by Graphite, chosen per context.
// Port of: src/gpu/graphite/RendererProvider.h#L41-L70 (chrome/m156)
#[doc(alias = "skgpu::graphite::PathRendererStrategy")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRendererStrategy {
    /// `kTessellation`: tessellation and stencil-and-cover with MSAA.
    Tessellation,
    /// `kTessellationAndSmallAtlas`: tessellation, with small paths rasterized on the CPU into an
    /// atlas (`minPathSizeForMSAA` > 0).
    TessellationAndSmallAtlas,
    /// `kRasterAtlas`: every path is rasterized on the CPU and packed into an atlas.
    RasterAtlas,
    /// `kComputeAnalyticAA` (Vello, not built here).
    ComputeAnalyticAA,
    /// `kComputeMSAA16` (Vello, not built here).
    ComputeMSAA16,
    /// `kComputeMSAA8` (Vello, not built here).
    ComputeMSAA8,
    /// `kCPUSparseStripsMSAA8` (sparse strips, G17).
    CpuSparseStripsMsaa8,
}

/// The renderers of the steps ported so far, and the cover steps the tessellating renderers
/// share (`RendererProvider`, partial).
// Port of: src/gpu/graphite/RendererProvider.h (the members for the ported steps)
#[doc(alias = "skgpu::graphite::RendererProvider")]
#[derive(Debug)]
pub struct RendererProvider {
    /// `fStrategy`.
    strategy: PathRendererStrategy,
    /// `fCoverageMask`: the renderer of the coverage masks (path atlases and mask images).
    coverage_mask: Renderer,
    /// `fAnalyticRRect`.
    analytic_rrect: Renderer,
    /// `fVertices[2 * hasColor + hasTexCoords]`.
    vertices: [Renderer; 4],
    /// `fBitmapText[int(MaskFormat)]`.
    bitmap_text: [Renderer; 3],
    /// `fSDFText[bool isLCD]`.
    sdf_text: [Renderer; 2],
    /// `fPerEdgeAAQuad`.
    per_edge_aa_quad: Renderer,
    /// `fNonAABoundsFill`.
    non_aa_bounds_fill: Renderer,
    /// `fCircularArc`.
    circular_arc: Renderer,
    /// `fAnalyticBlur`.
    analytic_blur: Renderer,
    /// `fAnalyticRRectBlur`.
    analytic_rrect_blur: Renderer,
    /// `fMesh`.
    mesh: Renderer,
    /// `fConvexTessellatedWedges`.
    convex_tessellated_wedges: Renderer,
    /// `fStencilTessellatedCurves[2 * inverse + evenOdd]`, indexed by `PathFillType`.
    stencil_tessellated_curves: [Renderer; 4],
    /// `fStencilTessellatedWedges[2 * inverse + evenOdd]`, indexed by `PathFillType`.
    stencil_tessellated_wedges: [Renderer; 4],
    /// `fTessellatedStrokes[inverseFill]`.
    tessellated_strokes: [Renderer; 2],
    /// The regular cover step of the stencil-then-cover renderers (`coverFill`).
    cover_fill: Arc<dyn RenderStep>,
    /// The inverse cover step of the stencil-then-cover renderers (`coverInverse`).
    cover_inverse: Arc<dyn RenderStep>,
    /// `fRenderSteps`: the steps of all the renderers, indexed by `RenderStepID`.
    render_steps: [Option<Arc<dyn RenderStep>>; NUM_RENDER_STEPS],
}

impl RendererProvider {
    /// `RendererProvider(caps, bufferManager)` for the steps ported so far. `infinity_support` is
    /// `caps->shaderCaps()->fInfinitySupport`, which chooses the curve-type attribute of the
    /// tessellating steps.
    // Port of: src/gpu/graphite/RendererProvider.cpp#L87 (chrome/m156), the ported initializers
    #[must_use]
    pub fn new(
        layout: Layout,
        infinity_support: bool,
        buffer_manager: &mut StaticBufferManager,
    ) -> Self {
        Self::new_with_strategy(
            PathRendererStrategy::Tessellation,
            layout,
            infinity_support,
            buffer_manager,
        )
    }

    /// `RendererProvider(caps, bufferManager)`'s strategy choice: whether `strategy` can be used
    /// with `caps`.
    // Port of: src/gpu/graphite/RendererProvider.cpp#L45-L80 (chrome/m156)
    #[must_use]
    pub fn is_supported(strategy: PathRendererStrategy, caps: &dyn Caps) -> bool {
        match strategy {
            PathRendererStrategy::TessellationAndSmallAtlas => {
                if caps.min_path_size_for_msaa() <= 0.0 {
                    return false; // Disabled explicitly
                }
                // Must support kTessellation too
                !caps.avoid_msaa()
            }
            // This strategy requires MSAA, which will use a supported MSAA count returned by
            // Caps::getDefaultMSAASampleCount(target). When avoidMSAA() returns false, this should
            // always be at least 4x on Graphite's supported devices.
            PathRendererStrategy::Tessellation => !caps.avoid_msaa(),
            // The raster path atlas is currently always supported.
            PathRendererStrategy::RasterAtlas => true,
            // The Vello compute strategies need `SK_ENABLE_VELLO_SHADERS`, which is off here.
            // The Vello compute strategies need `SK_ENABLE_VELLO_SHADERS`, and the sparse strips
            // are not ported yet (G17).
            PathRendererStrategy::ComputeAnalyticAA
            | PathRendererStrategy::ComputeMSAA16
            | PathRendererStrategy::ComputeMSAA8
            | PathRendererStrategy::CpuSparseStripsMsaa8 => false,
        }
    }

    /// The strategy the constructor chooses from `caps`: the requested one if it is supported,
    /// otherwise by preference (vello > tessellation [with atlas] > raster atlas).
    // Port of: src/gpu/graphite/RendererProvider.cpp#L87-L109 (chrome/m156)
    #[must_use]
    pub fn choose_strategy(caps: &dyn Caps) -> PathRendererStrategy {
        if let Some(requested) = caps.requested_path_renderer_strategy()
            && Self::is_supported(requested, caps)
        {
            // Use the explicitly overridden strategy
            return requested;
        }
        // By default, prefer vello > tessellation [w/ atlas] > raster atlas
        if Self::is_supported(PathRendererStrategy::ComputeMSAA8, caps) {
            PathRendererStrategy::ComputeMSAA8
        } else if caps.avoid_msaa() {
            PathRendererStrategy::RasterAtlas
        } else if caps.min_path_size_for_msaa() > 0.0 {
            PathRendererStrategy::TessellationAndSmallAtlas
        } else {
            PathRendererStrategy::Tessellation
        }
    }

    /// `RendererProvider(caps, bufferManager)` with the strategy `choose_strategy(caps)` gives.
    // Port of: src/gpu/graphite/RendererProvider.cpp#L87 (chrome/m156)
    #[must_use]
    pub fn new_for_caps(
        caps: &dyn Caps,
        layout: Layout,
        infinity_support: bool,
        buffer_manager: &mut StaticBufferManager,
    ) -> Self {
        Self::new_with_strategy(
            Self::choose_strategy(caps),
            layout,
            infinity_support,
            buffer_manager,
        )
    }

    /// `RendererProvider(caps, bufferManager)` with `strategy` as the path renderer strategy.
    // Port of: src/gpu/graphite/RendererProvider.cpp#L87-L140 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_lines)] // mirrors the C++ constructor
    pub fn new_with_strategy(
        strategy: PathRendererStrategy,
        layout: Layout,
        infinity_support: bool,
        buffer_manager: &mut StaticBufferManager,
    ) -> Self {
        // The tessellation steps are always initialized, because they are the fallback of every
        // atlas'ing path renderer strategy.
        let convex_tessellated_wedges = single_step(
            Arc::new(TessellateWedgesRenderStep::new(
                layout,
                RenderStepID::TessellateWedges_Convex,
                infinity_support,
                DIRECT_DEPTH_LESS_PASS,
                buffer_manager,
            )),
            DrawTypeFlags::NON_SIMPLE_SHAPE,
        );
        let analytic_rrect = single_step(
            Arc::new(AnalyticRRectRenderStep::new(layout, buffer_manager)),
            DrawTypeFlags::ANALYTIC_RRECT,
        );
        let vertices = [
            vertices_renderer(layout, false, false),
            vertices_renderer(layout, false, true),
            vertices_renderer(layout, true, false),
            vertices_renderer(layout, true, true),
        ];
        // Port of: src/gpu/graphite/RendererProvider.cpp#L141-L163 (chrome/m156)
        let bitmap_text = [
            (MaskFormat::A8, DrawTypeFlags::BITMAP_TEXT_MASK),
            (MaskFormat::A565, DrawTypeFlags::BITMAP_TEXT_LCD),
            (MaskFormat::Argb, DrawTypeFlags::BITMAP_TEXT_COLOR),
        ]
        .map(|(format, draw_types)| {
            single_step(
                Arc::new(BitmapTextRenderStep::new(layout, format)),
                draw_types,
            )
        });
        let sdf_text = [
            single_step(
                Arc::new(SDFTextRenderStep::new(layout)),
                DrawTypeFlags::SDF_TEXT,
            ),
            single_step(
                Arc::new(SDFTextLCDRenderStep::new(layout)),
                DrawTypeFlags::SDF_TEXT_LCD,
            ),
        ];
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
        let analytic_blur = single_step(
            Arc::new(AnalyticBlurRenderStep::new(layout)),
            DrawTypeFlags::DROP_SHADOWS,
        );
        let analytic_rrect_blur = single_step(
            Arc::new(AnalyticRRectBlurRenderStep::new(layout, buffer_manager)),
            DrawTypeFlags::DROP_SHADOWS,
        );
        // Port of: src/gpu/graphite/RendererProvider.cpp#L202 (chrome/m156), `initFromStep(&fMesh)`
        let mesh = single_step(
            Arc::new(MeshRenderStep::new(layout)),
            DrawTypeFlags::DRAW_MESH,
        );

        // The coverage mask renderer is always initialized: it is used for mask filters even when
        // the path renderer strategy wouldn't use it to sample an atlas.
        let coverage_mask = single_step(
            Arc::new(CoverageMaskRenderStep::new(layout)),
            DrawTypeFlags::NON_SIMPLE_SHAPE | DrawTypeFlags::INTERNAL_COVERAGE_MASK,
        );

        // The tessellating path renderers that use stencil can share the cover steps.
        let cover_fill: Arc<dyn RenderStep> = Arc::new(CoverBoundsRenderStep::new(
            layout,
            RenderStepID::CoverBounds_RegularCover,
            REGULAR_COVER_PASS,
        ));
        let cover_inverse: Arc<dyn RenderStep> = Arc::new(CoverBoundsRenderStep::new(
            layout,
            RenderStepID::CoverBounds_InverseCover,
            INVERSE_COVER_PASS,
        ));
        let (stencil_tessellated_curves, stencil_tessellated_wedges) =
            stencil_tessellated_renderers(
                layout,
                infinity_support,
                buffer_manager,
                &cover_fill,
                &cover_inverse,
            );

        // Strokes: the fill variant is a single step, the inverse fill also uses the cover step.
        let tessellated_strokes = [
            single_step(
                Arc::new(TessellateStrokesRenderStep::new(
                    layout,
                    infinity_support,
                    false,
                )),
                DrawTypeFlags::NON_SIMPLE_SHAPE,
            ),
            {
                let stroke_inverse: Arc<dyn RenderStep> = Arc::new(
                    TessellateStrokesRenderStep::new(layout, infinity_support, true),
                );
                Renderer::new(
                    "TessellatedStrokesInverseFill",
                    DrawTypeFlags::NON_SIMPLE_SHAPE,
                    vec![stroke_inverse, Arc::clone(&cover_inverse)],
                )
            },
        ];

        let mut provider = Self {
            strategy,
            coverage_mask,
            analytic_rrect,
            vertices,
            bitmap_text,
            sdf_text,
            per_edge_aa_quad,
            non_aa_bounds_fill,
            circular_arc,
            analytic_blur,
            analytic_rrect_blur,
            mesh,
            convex_tessellated_wedges,
            stencil_tessellated_curves,
            stencil_tessellated_wedges,
            tessellated_strokes,
            cover_fill,
            cover_inverse,
            render_steps: std::array::from_fn(|_| None),
        };
        provider.collect_render_steps();
        provider
    }

    /// Fills `fRenderSteps`: every step of every renderer by its id (`assumeOwnership`).
    fn collect_render_steps(&mut self) {
        let all_renderers = [
            &self.coverage_mask,
            &self.analytic_rrect,
            &self.per_edge_aa_quad,
            &self.non_aa_bounds_fill,
            &self.circular_arc,
            &self.analytic_blur,
            &self.analytic_rrect_blur,
            &self.mesh,
            &self.convex_tessellated_wedges,
        ]
        .into_iter()
        .chain(&self.vertices)
        .chain(&self.bitmap_text)
        .chain(&self.sdf_text)
        .chain(&self.stencil_tessellated_curves)
        .chain(&self.stencil_tessellated_wedges)
        .chain(&self.tessellated_strokes);
        for step in all_renderers
            .flat_map(|renderer| renderer.steps().iter())
            .chain([&self.cover_fill, &self.cover_inverse])
        {
            // Renderers share some steps (the cover steps), which are the same object.
            self.render_steps[step.render_step_id() as usize]
                .get_or_insert_with(|| Arc::clone(step));
        }
    }

    /// `lookup(renderStepID)`: the step with the given id, or `None` for an invalid id and for the
    /// steps that are not ported yet (Skia always has one).
    // Port of: src/gpu/graphite/RendererProvider.h#L160-L162 (chrome/m156)
    #[must_use]
    pub fn lookup(&self, render_step_id: RenderStepID) -> Option<&Arc<dyn RenderStep>> {
        self.render_steps[render_step_id as usize].as_ref()
    }

    /// `pathRendererStrategy()`.
    // Port of: src/gpu/graphite/RendererProvider.h#L92 (chrome/m156)
    #[must_use]
    pub const fn path_renderer_strategy(&self) -> PathRendererStrategy {
        self.strategy
    }

    /// `coverageMask()`: the renderer of `CoverageMaskShape` draws.
    // Port of: src/gpu/graphite/RendererProvider.h (coverageMask)
    #[must_use]
    pub const fn coverage_mask(&self) -> &Renderer {
        &self.coverage_mask
    }

    /// `fAnalyticRRect`.
    // Port of: src/gpu/graphite/RendererProvider.h (fAnalyticRRect)
    #[must_use]
    pub const fn analytic_rrect(&self) -> &Renderer {
        &self.analytic_rrect
    }

    /// `fVertices[2 * hasColor + hasTexCoords]`.
    // Port of: src/gpu/graphite/RendererProvider.h (fVertices)
    #[must_use]
    pub const fn vertices(&self, has_color: bool, has_tex_coords: bool) -> &Renderer {
        &self.vertices[2 * (has_color as usize) + (has_tex_coords as usize)]
    }

    /// `bitmapText(useLCDText, format)`: the renderer of atlased bitmap text. 565 represents all
    /// LCD rendering, regardless of the texture format.
    // Port of: src/gpu/graphite/RendererProvider.h#L116-L124 (chrome/m156)
    #[must_use]
    pub fn bitmap_text(&self, use_lcd_text: bool, format: MaskFormat) -> &Renderer {
        // We use 565 here to represent all LCD rendering, regardless of texture format
        if use_lcd_text {
            return &self.bitmap_text[MaskFormat::A565 as usize];
        }
        debug_assert_ne!(format, MaskFormat::A565);
        &self.bitmap_text[format as usize]
    }

    /// `sdfText(useLCDText)`: the renderer of distance field text.
    // Port of: src/gpu/graphite/RendererProvider.h#L125 (chrome/m156)
    #[must_use]
    pub fn sdf_text(&self, use_lcd_text: bool) -> &Renderer {
        &self.sdf_text[usize::from(use_lcd_text)]
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

    /// `analyticBlur()`.
    // Port of: src/gpu/graphite/RendererProvider.h#L148 (chrome/m156)
    #[must_use]
    pub const fn analytic_blur(&self) -> &Renderer {
        &self.analytic_blur
    }

    /// `analyticRRectBlur()`.
    // Port of: src/gpu/graphite/RendererProvider.h#L149 (chrome/m156)
    #[must_use]
    pub const fn analytic_rrect_blur(&self) -> &Renderer {
        &self.analytic_rrect_blur
    }

    /// `mesh()`: the renderer of `SkMesh` draws.
    // Port of: src/gpu/graphite/RendererProvider.h#L131-L133 (chrome/m156)
    #[must_use]
    pub const fn mesh(&self) -> &Renderer {
        &self.mesh
    }

    /// `convexTessellatedWedges()`.
    // Port of: src/gpu/graphite/RendererProvider.h#L105 (chrome/m156)
    #[must_use]
    pub const fn convex_tessellated_wedges(&self) -> &Renderer {
        &self.convex_tessellated_wedges
    }

    /// `stencilTessellatedCurvesAndTris(fillType)`: the fill of a path with its curves and
    /// triangles, drawn into the stencil and then covered.
    // Port of: src/gpu/graphite/RendererProvider.h#L99-L101 (chrome/m156)
    #[must_use]
    pub fn stencil_tessellated_curves_and_tris(&self, fill_type: PathFillType) -> &Renderer {
        &self.stencil_tessellated_curves[fill_type as usize]
    }

    /// `stencilTessellatedWedges(fillType)`: the fill of a path with its wedges, drawn into the
    /// stencil and then covered.
    // Port of: src/gpu/graphite/RendererProvider.h#L102-L104 (chrome/m156)
    #[must_use]
    pub fn stencil_tessellated_wedges(&self, fill_type: PathFillType) -> &Renderer {
        &self.stencil_tessellated_wedges[fill_type as usize]
    }

    /// `tessellatedStrokes(inverseFill)`.
    // Port of: src/gpu/graphite/RendererProvider.h#L106-L108 (chrome/m156)
    #[must_use]
    pub fn tessellated_strokes(&self, inverse_fill: bool) -> &Renderer {
        &self.tessellated_strokes[usize::from(inverse_fill)]
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

/// The stencil-then-cover renderers of the four fill types, as `(curves and triangles, wedges)`.
/// Each even-odd or winding stencil step is shared by its regular and inverse renderers, as in
/// Skia; the index is `2 * inverse + evenOdd`, which is the `PathFillType` value.
// Port of: src/gpu/graphite/RendererProvider.cpp#L163-L244 (chrome/m156), the stencil loop
fn stencil_tessellated_renderers(
    layout: Layout,
    infinity_support: bool,
    buffer_manager: &mut StaticBufferManager,
    cover_fill: &Arc<dyn RenderStep>,
    cover_inverse: &Arc<dyn RenderStep>,
) -> ([Renderer; 4], [Renderer; 4]) {
    // The variants of `kTessVariants`, indexed like the renderers.
    const TESS_VARIANTS: [&str; 4] = [
        "[winding]",
        "[evenodd]",
        "[inverse-winding]",
        "[inverse-evenodd]",
    ];

    let mut curves: [Option<Renderer>; 4] = [None, None, None, None];
    let mut wedges: [Option<Renderer>; 4] = [None, None, None, None];
    for even_odd in [false, true] {
        // These steps can be shared by regular and inverse fills.
        let stencil_fan: Arc<dyn RenderStep> =
            Arc::new(MiddleOutFanRenderStep::new(layout, even_odd));
        let stencil_curve: Arc<dyn RenderStep> = Arc::new(TessellateCurvesRenderStep::new(
            layout,
            even_odd,
            infinity_support,
            buffer_manager,
        ));
        let stencil_wedge: Arc<dyn RenderStep> = if even_odd {
            Arc::new(TessellateWedgesRenderStep::new(
                layout,
                RenderStepID::TessellateWedges_EvenOdd,
                infinity_support,
                EVEN_ODD_STENCIL_PASS,
                buffer_manager,
            ))
        } else {
            Arc::new(TessellateWedgesRenderStep::new(
                layout,
                RenderStepID::TessellateWedges_Winding,
                infinity_support,
                WINDING_STENCIL_PASS,
                buffer_manager,
            ))
        };
        for inverse in [false, true] {
            let index = 2 * usize::from(inverse) + usize::from(even_odd);
            let cover_step = if inverse { cover_inverse } else { cover_fill };
            let variant = TESS_VARIANTS[index];
            curves[index] = Some(Renderer::new(
                &format!("StencilTessellatedCurvesAndTris{variant}"),
                DrawTypeFlags::NON_SIMPLE_SHAPE,
                vec![
                    Arc::clone(&stencil_fan),
                    Arc::clone(&stencil_curve),
                    Arc::clone(cover_step),
                ],
            ));
            wedges[index] = Some(Renderer::new(
                &format!("StencilTessellatedWedges{variant}"),
                DrawTypeFlags::NON_SIMPLE_SHAPE,
                vec![Arc::clone(&stencil_wedge), Arc::clone(cover_step)],
            ));
        }
    }
    (
        curves.map(|r| r.expect("every fill type has a stencil curve renderer")),
        wedges.map(|r| r.expect("every fill type has a stencil wedge renderer")),
    )
}

/// One `VerticesRenderStep` variant as its renderer (`fVertices[2 * hasColor + hasTexCoords]`).
// Port of: src/gpu/graphite/RendererProvider.cpp#L192-L202 (chrome/m156)
fn vertices_renderer(layout: Layout, has_color: bool, has_tex_coords: bool) -> Renderer {
    // DropShadows is added to the color-only variant, which Android uses for drop shadows.
    let mut draw_types = DrawTypeFlags::DRAW_VERTICES;
    if has_color && !has_tex_coords {
        draw_types |= DrawTypeFlags::DROP_SHADOWS;
    }
    single_step(
        Arc::new(VerticesRenderStep::new(layout, has_color, has_tex_coords)),
        draw_types,
    )
}

/// `initFromStep`: a renderer made of one step, named `SingleStep[<step name>]`. Single-step
/// renderers do not share their step.
// Port of: src/gpu/graphite/RendererProvider.cpp#L116-L125 (chrome/m156)
fn single_step(step: Arc<dyn RenderStep>, draw_types: DrawTypeFlags) -> Renderer {
    let name = format!("SingleStep[{}]", step.name());
    Renderer::new(&name, draw_types, vec![step])
}
