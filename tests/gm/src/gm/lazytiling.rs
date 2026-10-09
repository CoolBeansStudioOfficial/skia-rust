// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lazytiling.cpp (chrome/m156)

// This GM exercises all the different tile modes for a texture that cannot be normalized early
// (i.e., rectangle or fully-lazy). It is a GpuGM: its GPU setup and drawing need a direct GPU
// context, which a raster canvas never has, so on the raster sinks it is skipped
// (`onGpuSetup` and `onDraw` return kSkip, as in the C++ when `GrAsDirectContext` is null).
// The texture creation and the tile-mode grid are GPU-only and are not ported.

use crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY;
use crate::prelude::*;

// Port of: gm/lazytiling.cpp(chrome/m156), the GM's constants
const PAD: i32 = 4;
const CONTENT_SIZE: i32 = 32;
// `kSkTileModeCount`: clamp, repeat, mirror, decal.
const SK_TILE_MODE_COUNT: i32 = 4;
const TOTAL_WIDTH: i32 = (2 * CONTENT_SIZE + PAD) * SK_TILE_MODE_COUNT + PAD;
const TOTAL_HEIGHT: i32 = (2 * CONTENT_SIZE + PAD) * SK_TILE_MODE_COUNT + PAD;

// Port of: gm/lazytiling.cpp(chrome/m156), class LazyTilingGM
#[derive(Debug)]
pub struct LazyTilingGm {
    top_left: bool,
}

impl LazyTilingGm {
    // Port of: gm/lazytiling.cpp(chrome/m156), LazyTilingGM(GrSurfaceOrigin)
    #[must_use]
    pub fn new(top_left: bool) -> Self {
        Self { top_left }
    }
}

impl GM for LazyTilingGm {
    // Port of: gm/lazytiling.cpp(chrome/m156), getName
    fn name(&self) -> String {
        format!("lazytiling_{}", if self.top_left { "tl" } else { "bl" })
    }

    fn size(&mut self) -> ISize {
        ISize::new(TOTAL_WIDTH, TOTAL_HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::from_argb(0xFF, 0xCC, 0xCC, 0xCC)
    }

    // Port of: gm/lazytiling.cpp(chrome/m156), onGpuSetup
    fn on_gpu_setup(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        // A raster canvas has no direct GPU context: kSkip.
        DrawResult::Skip
    }

    // Port of: gm/lazytiling.cpp(chrome/m156), onDraw
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        ERROR_MSG_DRAW_SKIPPED_GPU_ONLY.clone_into(error_msg);
        DrawResult::Skip
    }
}

// Port of: gm/lazytiling.cpp (chrome/m156)
crate::def_gm!(
    LazyTilingGM_kBottomLeft_GrSurfaceOrigin = "LazyTilingGM(kBottomLeft_GrSurfaceOrigin)",
    LazyTilingGm::new(false)
);
crate::def_gm!(
    LazyTilingGM_kTopLeft_GrSurfaceOrigin = "LazyTilingGM(kTopLeft_GrSurfaceOrigin)",
    LazyTilingGm::new(true)
);
