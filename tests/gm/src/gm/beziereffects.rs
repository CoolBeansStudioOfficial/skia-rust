// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/beziereffects.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): both GMs are `GpuGM`s that need the Ganesh
// surface draw context (`TopDeviceSurfaceDrawContext`). On a raster sink there is none, so they
// skip before any drawing; the Ganesh-only effect drawing is not ported, as it can never run here.

use crate::prelude::*;

const CELL_WIDTH: i32 = 128;
const CELL_HEIGHT: i32 = 128;

// Port of: gm/beziereffects.cpp#L214-L304 (chrome/m156), BezierConicEffects
struct BezierConicEffectsGm;

impl GM for BezierConicEffectsGm {
    fn name(&self) -> String {
        "bezier_conic_effects".to_string()
    }

    fn size(&mut self) -> ISize {
        const NUM_CONICS: i32 = 10;
        ISize::new(CELL_WIDTH, NUM_CONICS * CELL_HEIGHT)
    }

    // Port of: gm/beziereffects.cpp#L229-L234 (chrome/m156), the GPU-only skip
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        error_msg.clear();
        error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
        DrawResult::Skip
    }
}

// Port of: gm/beziereffects.cpp#L412-L498 (chrome/m156), BezierQuadEffects
struct BezierQuadEffectsGm;

impl GM for BezierQuadEffectsGm {
    fn name(&self) -> String {
        "bezier_quad_effects".to_string()
    }

    fn size(&mut self) -> ISize {
        const NUM_QUADS: i32 = 5;
        ISize::new(CELL_WIDTH, NUM_QUADS * CELL_HEIGHT)
    }

    // Port of: gm/beziereffects.cpp#L427-L432 (chrome/m156), the GPU-only skip
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        error_msg.clear();
        error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
        DrawResult::Skip
    }
}

// Port of: gm/beziereffects.cpp#L505-L506 (chrome/m156)
crate::def_gm!(BezierConicEffects, BezierConicEffectsGm);
crate::def_gm!(BezierQuadEffects, BezierQuadEffectsGm);
