// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/GraphicsPipelineDesc.h

//! [`GraphicsPipelineDesc`]: what identifies a graphics pipeline besides its render pass.

use crate::graphite::render_step::RenderStepID;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

/// `GraphicsPipelineDesc` represents the state needed to create a backend specific
/// `GraphicsPipeline`, minus the target-specific properties that can be inferred from the
/// `DrawPass` and `RenderPassTask`.
///
/// The `GPU_TEST_UTILS` `toString(caps, dict)` is not ported: it needs the paint key's
/// `toString`, which the dictionary does not expose for a bare `UniquePaintParamsID` yet.
// Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L19-L58 (chrome/m156)
#[doc(alias = "skgpu::graphite::GraphicsPipelineDesc")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GraphicsPipelineDesc {
    /// Each `RenderStep` defines a fixed set of attributes and rasterization state, as well as the
    /// shader fragments that control the geometry and coverage calculations. The `RenderStep`'s
    /// shader is combined with the rest of the shader generated from the `PaintParams`. Because
    /// each `RenderStep` is fixed, its id can be used as a proxy for everything that it specifies
    /// in the `GraphicsPipeline`.
    render_step_id: RenderStepID,
    /// `fPaintID`.
    paint_id: UniquePaintParamsID,
}

impl Default for GraphicsPipelineDesc {
    /// `GraphicsPipelineDesc()`: an invalid render step and paint.
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L24-L25 (chrome/m156)
    fn default() -> Self {
        Self {
            render_step_id: RenderStepID::Invalid,
            paint_id: UniquePaintParamsID::invalid(),
        }
    }
}

impl GraphicsPipelineDesc {
    /// `GraphicsPipelineDesc(renderStepID, paintID)`.
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L26-L28 (chrome/m156)
    #[must_use]
    pub const fn new(render_step_id: RenderStepID, paint_id: UniquePaintParamsID) -> Self {
        Self {
            render_step_id,
            paint_id,
        }
    }

    /// `renderStepID()`: describes the geometric portion of the pipeline's program and the
    /// pipeline's fixed state (except for renderpass-level state that will never change between
    /// draws).
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L40-L41 (chrome/m156)
    #[doc(alias = "renderStepID")]
    #[must_use]
    pub const fn render_step_id(&self) -> RenderStepID {
        self.render_step_id
    }

    /// `paintParamsID()`: the unique id of the required `PaintParams`.
    // Port of: src/gpu/graphite/GraphicsPipelineDesc.h#L43-L43 (chrome/m156)
    #[doc(alias = "paintParamsID")]
    #[must_use]
    pub const fn paint_params_id(&self) -> UniquePaintParamsID {
        self.paint_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_desc_is_invalid() {
        let desc = GraphicsPipelineDesc::default();
        assert_eq!(desc.render_step_id(), RenderStepID::Invalid);
        assert!(!desc.paint_params_id().is_valid());
    }

    #[test]
    fn descs_compare_by_step_and_paint() {
        let a = GraphicsPipelineDesc::new(RenderStepID::CircularArc, UniquePaintParamsID::new(3));
        assert_eq!(
            a,
            GraphicsPipelineDesc::new(RenderStepID::CircularArc, UniquePaintParamsID::new(3))
        );
        assert_ne!(
            a,
            GraphicsPipelineDesc::new(RenderStepID::CircularArc, UniquePaintParamsID::new(4))
        );
        assert_ne!(
            a,
            GraphicsPipelineDesc::new(RenderStepID::AnalyticRRect, UniquePaintParamsID::new(3))
        );
    }
}
