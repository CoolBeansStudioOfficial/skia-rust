// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bigrrectaaeffect.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): `BigRRectAAEffectGM` is a `GpuGM` that needs the
// Ganesh surface draw context. On a raster sink there is none, so it skips before any drawing; the
// Ganesh-only rrect effect drawing is not ported, as it can never run here. Only the surface size
// is computed, from the C++ constructor's arithmetic.

use crate::prelude::*;

const K_GAP: i32 = 3;
const K_PAD: i32 = 7;

// Port of: gm/bigrrectaaeffect.cpp#L42-L120 (chrome/m156), BigRRectAAEffectGM
struct BigRRectAaEffectGm {
    name: &'static str,
    width: i32,
    height: i32,
}

impl BigRRectAaEffectGm {
    // Port of: gm/bigrrectaaeffect.cpp#L45-L61 (chrome/m156), the constructor's sizes.
    // `rrect_width`/`rrect_height` are `SkScalarCeilToInt` of the rrect's extent.
    fn new(rrect_width: i32, rrect_height: i32, name: &'static str) -> Self {
        // Each test case draws the rrect with gaps around it.
        let test_width = rrect_width + 2 * K_GAP;
        let test_height = rrect_height + 2 * K_GAP;
        // Add a pad between test cases.
        let test_offset_x = test_width + K_PAD;
        let test_offset_y = test_height + K_PAD;
        // Two tests in x (fill and inv-fill) and a pad around all four sides.
        Self {
            name,
            width: 2 * test_offset_x + K_PAD,
            height: test_offset_y + K_PAD,
        }
    }
}

impl GM for BigRRectAaEffectGm {
    fn name(&self) -> String {
        format!("big_rrect_{}_aa_effect", self.name)
    }

    fn size(&mut self) -> ISize {
        ISize::new(self.width, self.height)
    }

    // Port of: gm/bigrrectaaeffect.cpp#L47 (chrome/m156), setBGColor(ToolUtils::color_to_565(SK_ColorBLUE))
    fn bg_color(&self) -> Color {
        crate::tool_utils::color_to_565(Color::BLUE)
    }

    // Port of: gm/bigrrectaaeffect.cpp#L71-L76 (chrome/m156), the GPU-only skip
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        error_msg.clear();
        error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
        DrawResult::Skip
    }
}

// Port of: gm/bigrrectaaeffect.cpp#L143-L148 (chrome/m156). `kSize` is 700; the rrect extents are
// `kSize - 1` by `kSize - 10` (699 x 690) or `kSize` by `kSize` (700 x 700).
crate::def_gm!(
    BigRRectAAEffectGM_rect =
        "BigRRectAAEffectGM (SkRRect::MakeRect(SkRect::MakeIWH(kSize, kSize)), \"rect\")",
    BigRRectAaEffectGm::new(700, 700, "rect")
);
crate::def_gm!(
    BigRRectAAEffectGM_circle =
        "BigRRectAAEffectGM (SkRRect::MakeOval(SkRect::MakeIWH(kSize, kSize)), \"circle\")",
    BigRRectAaEffectGm::new(700, 700, "circle")
);
crate::def_gm!(
    BigRRectAAEffectGM_ellipse = "BigRRectAAEffectGM (SkRRect::MakeOval(SkRect::MakeIWH(kSize - 1, kSize - 10)), \"ellipse\")",
    BigRRectAaEffectGm::new(699, 690, "ellipse")
);
crate::def_gm!(
    BigRRectAAEffectGM_circular_corner = "BigRRectAAEffectGM (SkRRect::MakeRectXY(SkRect::MakeIWH(kSize - 1, kSize - 10), kSize/2.f - 10.f, kSize/2.f - 10.f), \"circular_corner\")",
    BigRRectAaEffectGm::new(699, 690, "circular_corner")
);
crate::def_gm!(
    BigRRectAAEffectGM_elliptical_corner = "BigRRectAAEffectGM (SkRRect::MakeRectXY(SkRect::MakeIWH(kSize - 1, kSize - 10), kSize/2.f - 10.f, kSize/2.f - 15.f), \"elliptical_corner\")",
    BigRRectAaEffectGm::new(699, 690, "elliptical_corner")
);
