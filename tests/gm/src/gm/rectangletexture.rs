// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rectangletexture.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): this GM is a GPU-only test. Its `onGpuSetup`
// returns `DrawResult::Skip` when there is no GPU context, which is always the case for a raster
// sink, so nothing is drawn. The rest of the C++ (the OpenGL rectangle-texture images and
// `onDraw`) can never run here and is not ported.

use crate::prelude::*;

// Port of: gm/rectangletexture.cpp#L45-L59 (chrome/m156), RectangleTexture (the skip path only)
struct RectangleTextureGm;

impl GM for RectangleTextureGm {
    // Port of: gm/rectangletexture.cpp#L57 (chrome/m156), getName
    fn name(&self) -> String {
        "rectangle_texture".to_owned()
    }

    // Port of: gm/rectangletexture.cpp#L59 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1180, 710)
    }

    // Port of: gm/rectangletexture.cpp#L116-L120 (chrome/m156), onGpuSetup: no GPU context
    fn on_gpu_setup(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        DrawResult::Skip
    }

    fn on_draw_with_error(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        panic!("onGpuSetup skips on a raster sink, so onDraw never runs");
    }
}

// Port of: gm/rectangletexture.cpp#L258 (chrome/m156)
crate::def_gm!(RectangleTexture, RectangleTextureGm);
