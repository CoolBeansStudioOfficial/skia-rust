// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/texelsubset.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): `TexelSubset` is a `GpuGM` that needs the Ganesh
// surface draw context (`TopDeviceSurfaceDrawContext`). On a raster sink there is none, so it skips
// before any drawing; the Ganesh-only texel-subset drawing is not ported, as it can never run here.

use crate::prelude::*;

// Port of: gm/texelsubset.cpp#L30-L46 (chrome/m156), the `Filter` and `MipmapMode` enums
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MipmapMode {
    None,
    Nearest,
    Linear,
}

const K_TEST_PAD: i32 = 10;
const K_DRAW_PAD: i32 = 10;
const K_IMAGE_WIDTH: i32 = 128;
const K_IMAGE_HEIGHT: i32 = 88;
// GrSamplerState::kWrapModeCount: clamp, repeat, mirror repeat and clamp to border.
const K_WRAP_MODE_COUNT: i32 = 4;

// Port of: gm/texelsubset.cpp#L47-L240 (chrome/m156), TexelSubset
struct TexelSubsetGm {
    filter: Filter,
    mipmap_mode: MipmapMode,
    // Read only by the Ganesh drawing (not ported): it selects the upscaled subset draw.
    #[allow(dead_code)]
    upscale: bool,
}

impl TexelSubsetGm {
    // Port of: gm/texelsubset.cpp#L49-L53 (chrome/m156), the constructor
    fn new(filter: Filter, mipmap_mode: MipmapMode, upscale: bool) -> Self {
        Self {
            filter,
            mipmap_mode,
            upscale,
        }
    }
}

impl GM for TexelSubsetGm {
    // Port of: gm/texelsubset.cpp#L55-L77 (chrome/m156), getName
    fn name(&self) -> String {
        let mut name = String::from("texel_subset");
        match self.filter {
            Filter::Nearest => name.push_str("_nearest"),
            Filter::Linear => name.push_str("_linear"),
        }
        match self.mipmap_mode {
            MipmapMode::None => {}
            MipmapMode::Nearest => name.push_str("_mipmap_nearest"),
            MipmapMode::Linear => name.push_str("_mipmap_linear"),
        }
        name
    }

    // Port of: gm/texelsubset.cpp#L79-L85 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        let n = K_WRAP_MODE_COUNT;
        let w = K_TEST_PAD + 2 * n * (K_IMAGE_WIDTH + 2 * K_DRAW_PAD + K_TEST_PAD);
        let h = K_TEST_PAD + 2 * n * (K_IMAGE_HEIGHT + 2 * K_DRAW_PAD + K_TEST_PAD);
        ISize::new(w, h)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFFF_FFFF)
    }

    // Port of: gm/texelsubset.cpp#L93-L98 (chrome/m156), the GPU-only skip
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        error_msg.clear();
        error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
        DrawResult::Skip
    }
}

// Port of: gm/texelsubset.cpp#L241-L249 (chrome/m156)
crate::def_gm!(
    TexelSubset_NN_F = "TexelSubset(Filter::kNearest, MipmapMode::kNone , false)",
    TexelSubsetGm::new(Filter::Nearest, MipmapMode::None, false)
);
crate::def_gm!(
    TexelSubset_NN_T = "TexelSubset(Filter::kNearest, MipmapMode::kNone , true )",
    TexelSubsetGm::new(Filter::Nearest, MipmapMode::None, true)
);
crate::def_gm!(
    TexelSubset_LN_F = "TexelSubset(Filter::kLinear , MipmapMode::kNone , false)",
    TexelSubsetGm::new(Filter::Linear, MipmapMode::None, false)
);
crate::def_gm!(
    TexelSubset_LN_T = "TexelSubset(Filter::kLinear , MipmapMode::kNone , true )",
    TexelSubsetGm::new(Filter::Linear, MipmapMode::None, true)
);
crate::def_gm!(
    TexelSubset_NNe_F = "TexelSubset(Filter::kNearest, MipmapMode::kNearest, false)",
    TexelSubsetGm::new(Filter::Nearest, MipmapMode::Nearest, false)
);
crate::def_gm!(
    TexelSubset_LNe_F = "TexelSubset(Filter::kLinear , MipmapMode::kNearest, false)",
    TexelSubsetGm::new(Filter::Linear, MipmapMode::Nearest, false)
);
crate::def_gm!(
    TexelSubset_NL_F = "TexelSubset(Filter::kNearest, MipmapMode::kLinear , false)",
    TexelSubsetGm::new(Filter::Nearest, MipmapMode::Linear, false)
);
crate::def_gm!(
    TexelSubset_LL_F = "TexelSubset(Filter::kLinear , MipmapMode::kLinear , false)",
    TexelSubsetGm::new(Filter::Linear, MipmapMode::Linear, false)
);
