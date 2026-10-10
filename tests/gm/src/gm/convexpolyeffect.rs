// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/convexpolyeffect.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): `ConvexPolyEffect` is a `GpuGM` that needs the
// Ganesh surface draw context (`TopDeviceSurfaceDrawContext`). On a raster sink there is none, so
// it skips before any drawing; the Ganesh-only drawing is not ported, as it can never run here.

use crate::prelude::*;

// Port of: gm/convexpolyeffect.cpp#L43-L136 (chrome/m156), ConvexPolyEffect
struct ConvexPolyEffectGm;

impl GM for ConvexPolyEffectGm {
    fn name(&self) -> String {
        "convex_poly_effect".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(720, 550)
    }

    // Port of: gm/convexpolyeffect.cpp#L88-L93 (chrome/m156), the GPU-only skip
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        error_msg.clear();
        error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
        DrawResult::Skip
    }
}

// Port of: gm/convexpolyeffect.cpp#L145 (chrome/m156)
crate::def_gm!(ConvexPolyEffect, ConvexPolyEffectGm);
