// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/aarecteffect.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): `AARectEffect` is a `GpuGM` that needs the
// Ganesh surface draw context (`TopDeviceSurfaceDrawContext`). On a raster sink there is none, so
// `GpuGM::onDraw` reports `kErrorMsg_DrawSkippedGpuOnly` and skips before any drawing. The
// Ganesh-only drawing that follows is therefore not ported, as it can never run here.

use crate::prelude::*;

// Port of: gm/aarecteffect.cpp#L40-L115 (chrome/m156), AARectEffect
struct AaRectEffectGm;

impl GM for AaRectEffectGm {
    fn name(&self) -> String {
        "aa_rect_effect".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(210, 250)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFFF_FFFF)
    }

    // Port of: gm/aarecteffect.cpp#L63-L70 (chrome/m156), the GPU-only skip
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        error_msg.clear();
        error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
        DrawResult::Skip
    }
}

// Port of: gm/aarecteffect.cpp#L117 (chrome/m156)
crate::def_gm!(AARectEffect, AaRectEffectGm);
