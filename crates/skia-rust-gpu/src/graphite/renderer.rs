// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Renderer.h (Renderer)

//! [`Renderer`]: the technique for one kind of draw, as a list of [`RenderStep`]s.

use std::sync::Arc;

use crate::graphite::draw_types::DrawTypeFlags;
use crate::graphite::graphite_types::DepthStencilFlags;
use crate::graphite::render_step::{Coverage, RenderStep, RenderStepFlags, get_coverage};

/// The maximum number of render steps any `Renderer` may have (`Renderer::kMaxRenderSteps`).
// Port of: src/gpu/graphite/Renderer.h#L281 (chrome/m156)
#[doc(alias = "kMaxRenderSteps")]
pub const MAX_RENDER_STEPS: usize = 4;

/// A technique that decomposes a draw into `RenderStep`s, run in order. Each `Renderer` is a
/// singleton held by the `RendererProvider`. Renderers are not virtual: they point at the steps,
/// which are shared between renderers.
// Port of: src/gpu/graphite/Renderer.h#L229-L281 (chrome/m156)
#[doc(alias = "skgpu::graphite::Renderer")]
#[derive(Clone, Debug)]
pub struct Renderer {
    steps: Vec<Arc<dyn RenderStep>>,
    name: String,
    draw_types: DrawTypeFlags,
    step_flags: RenderStepFlags,
    depth_stencil_flags: DepthStencilFlags,
}

impl Renderer {
    /// `Renderer(name, drawTypes, steps...)`. The steps are shared (`Arc`) because a step can be
    /// used by several renderers. At least one step must shade, and a step that uses non-AA
    /// inner fills must be the only step, with a `LESS` depth test.
    ///
    /// # Panics
    /// If the number of steps is not between 1 and `kMaxRenderSteps`.
    // Port of: src/gpu/graphite/Renderer.h#L247-L268 (chrome/m156)
    #[must_use]
    pub fn new(name: &str, draw_types: DrawTypeFlags, steps: Vec<Arc<dyn RenderStep>>) -> Self {
        assert!(
            (1..=MAX_RENDER_STEPS).contains(&steps.len()),
            "a Renderer has between 1 and kMaxRenderSteps steps"
        );
        let mut step_flags = RenderStepFlags::NONE;
        let mut depth_stencil_flags = DepthStencilFlags::None;
        for step in &steps {
            step_flags |= step.base().flags();
            depth_stencil_flags = or_depth_stencil_flags(
                depth_stencil_flags,
                depth_stencil_flags_of(step.base().depth_stencil_settings()),
            );
        }
        // At least one step needs to actually shade.
        debug_assert!(step_flags.contains(RenderStepFlags::PERFORMS_SHADING));
        // A step using non-AA inner fills with a second draw must not be part of a multi-step
        // renderer, and must use the LESS depth test.
        debug_assert!(
            !step_flags.contains(RenderStepFlags::USE_NON_AA_INNER_FILL)
                || (steps.len() == 1
                    && steps[0].base().depth_stencil_settings().depth_test_enabled
                    && steps[0].base().depth_stencil_settings().depth_compare_op
                        == crate::graphite::draw_types::CompareOp::Less)
        );
        Self {
            steps,
            name: name.to_owned(),
            draw_types,
            step_flags,
            depth_stencil_flags,
        }
    }

    /// `step(i)`.
    ///
    /// # Panics
    /// If `i` is not a step of this renderer.
    // Port of: src/gpu/graphite/Renderer.h#L233-L237 (chrome/m156)
    #[must_use]
    pub fn step(&self, i: usize) -> &dyn RenderStep {
        self.steps[i].as_ref()
    }

    /// `steps()`.
    // Port of: src/gpu/graphite/Renderer.h#L238-L241 (chrome/m156)
    #[must_use]
    pub fn steps(&self) -> &[Arc<dyn RenderStep>] {
        &self.steps
    }

    /// `name()`.
    // Port of: src/gpu/graphite/Renderer.h#L243 (chrome/m156)
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `drawTypes()`.
    // Port of: src/gpu/graphite/Renderer.h#L244 (chrome/m156)
    #[must_use]
    pub const fn draw_types(&self) -> DrawTypeFlags {
        self.draw_types
    }

    /// `numRenderSteps()`.
    // Port of: src/gpu/graphite/Renderer.h#L245 (chrome/m156)
    #[must_use]
    pub fn num_render_steps(&self) -> usize {
        self.steps.len()
    }

    /// `requiresMSAA()`.
    // Port of: src/gpu/graphite/Renderer.h#L246-L248 (chrome/m156)
    #[must_use]
    pub fn requires_msaa(&self) -> bool {
        self.step_flags.contains(RenderStepFlags::REQUIRES_MSAA)
    }

    /// `emitsPrimitiveColor()`.
    // Port of: src/gpu/graphite/Renderer.h#L249-L251 (chrome/m156)
    #[must_use]
    pub fn emits_primitive_color(&self) -> bool {
        self.step_flags
            .contains(RenderStepFlags::EMITS_PRIMITIVE_COLOR)
    }

    /// `outsetBoundsForAA()`.
    // Port of: src/gpu/graphite/Renderer.h#L252-L254 (chrome/m156)
    #[must_use]
    pub fn outset_bounds_for_aa(&self) -> bool {
        self.step_flags
            .contains(RenderStepFlags::OUTSET_BOUNDS_FOR_AA)
    }

    /// `useNonAAInnerFill()`.
    // Port of: src/gpu/graphite/Renderer.h#L255-L257 (chrome/m156)
    #[must_use]
    pub fn use_non_aa_inner_fill(&self) -> bool {
        self.step_flags
            .contains(RenderStepFlags::USE_NON_AA_INNER_FILL)
    }

    /// `depthStencilFlags()`.
    // Port of: src/gpu/graphite/Renderer.h#L259 (chrome/m156)
    #[must_use]
    pub const fn depth_stencil_flags(&self) -> DepthStencilFlags {
        self.depth_stencil_flags
    }

    /// `coverage()`.
    // Port of: src/gpu/graphite/Renderer.h#L260 (chrome/m156)
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        get_coverage(self.step_flags)
    }
}

// Port of: src/gpu/graphite/Renderer.h#L226-L228 (chrome/m156), `fDepthStencilFlags |=`
pub(crate) fn or_depth_stencil_flags(
    a: DepthStencilFlags,
    b: DepthStencilFlags,
) -> DepthStencilFlags {
    let bits = |f: DepthStencilFlags| match f {
        DepthStencilFlags::None => 0_u8,
        DepthStencilFlags::Depth => 1,
        DepthStencilFlags::Stencil => 2,
        DepthStencilFlags::DepthStencil => 3,
    };
    match bits(a) | bits(b) {
        0 => DepthStencilFlags::None,
        1 => DepthStencilFlags::Depth,
        2 => DepthStencilFlags::Stencil,
        _ => DepthStencilFlags::DepthStencil,
    }
}

// Port of: src/gpu/graphite/DrawTypes.h (DepthStencilSettings), the `depthStencilFlags()` of
// RenderStep: stencil if the stencil test is on, depth if depth testing or writing is on.
pub(crate) fn depth_stencil_flags_of(
    settings: &crate::graphite::draw_types::DepthStencilSettings,
) -> DepthStencilFlags {
    let stencil = if settings.stencil_test_enabled {
        DepthStencilFlags::Stencil
    } else {
        DepthStencilFlags::None
    };
    let depth = if settings.depth_test_enabled || settings.depth_write_enabled {
        DepthStencilFlags::Depth
    } else {
        DepthStencilFlags::None
    };
    or_depth_stencil_flags(stencil, depth)
}
