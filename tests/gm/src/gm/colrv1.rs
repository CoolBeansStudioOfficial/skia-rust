// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colrv1.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): every COLRv1 test font is a resource, so
// `ToolUtils::CreateTypefaceFromResource` is `None` in the portable configuration, and each GM
// returns `DrawResult::Skip` in `onDraw`, as it did when the goldens were made. The drawing
// code is not ported, because it can never run here; a non-null typeface panics.

use crate::prelude::*;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::create_typeface_from_resource;
/// `ColrV1GM`: the `COLRv1` test GM for one test name, skew, rotation and variation set.
// Port of: gm/colrv1.cpp#L43-L183 (chrome/m156), ColrV1GM (the skip path only)
struct ColrV1Gm {
    test_name: &'static str,
    skew_x: f32,
    rotate_deg: f32,
    /// The variation coordinates: `(tag, value)`.
    variations: &'static [(&'static [u8; 4], f32)],
    typeface: Option<Typeface>,
}

impl ColrV1Gm {
    // Port of: gm/colrv1.cpp#L52-L62 (chrome/m156), the constructor
    const fn new(
        test_name: &'static str,
        skew_x: f32,
        rotate_deg: f32,
        variations: &'static [(&'static [u8; 4], f32)],
    ) -> Self {
        Self {
            test_name,
            skew_x,
            rotate_deg,
            variations,
            typeface: None,
        }
    }
}

impl GM for ColrV1Gm {
    // Port of: gm/colrv1.cpp#L72-L92 (chrome/m156), getName
    fn name(&self) -> String {
        let mut gm_name = format!("colrv1_{}", self.test_name);
        if self.skew_x != 0.0 {
            gm_name = format!("{gm_name}_skew_{:.2}", self.skew_x);
        }
        if self.rotate_deg != 0.0 {
            gm_name = format!("{gm_name}_rotate_{:.2}", self.rotate_deg);
        }
        for (tag, value) in self.variations {
            let tag_name: String = tag.iter().map(|&b| char::from(b)).collect();
            gm_name = format!("{gm_name}_{tag_name}_{value:.2}");
        }
        gm_name
    }

    fn size(&mut self) -> ISize {
        ISize::new(650, 1200)
    }

    // Port of: gm/colrv1.cpp#L63-L70 (chrome/m156), onOnceBeforeDraw. Both resources are
    // missing in the portable configuration, so the typeface is always `None`.
    fn on_once_before_draw(&mut self) {
        self.typeface = create_typeface_from_resource(None, 0);
    }

    // Port of: gm/colrv1.cpp#L120-L131 (chrome/m156), onDraw: the skip path
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        canvas.draw_color(Color::WHITE, None);
        if self.typeface.is_none() {
            "Did not recognize COLR v1 font format.".clone_into(error_msg);
            return DrawResult::Skip;
        }
        panic!("a COLRv1 typeface is never loaded in the portable configuration");
    }
}

// Port of: gm/colrv1.cpp#L249 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_clipbox___0_0f__0_0f_____ = "F(C(clipbox), 0.0f, 0.0f, {})",
    ColrV1Gm::new("clipbox", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L251 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_composite_mode___0_0f__0_0f_____ = "F(C(composite_mode), 0.0f, 0.0f, {})",
    ColrV1Gm::new("composite_mode", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L252 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_composite_mode____0_5f__0_0f_____ = "F(C(composite_mode), -0.5f, 0.0f, {})",
    ColrV1Gm::new("composite_mode", -0.5f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L253 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_composite_mode____0_5f__20_0f_____ = "F(C(composite_mode), -0.5f, 20.0f, {})",
    ColrV1Gm::new("composite_mode", -0.5f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L254 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_composite_mode___0_0f__20_0f_____ = "F(C(composite_mode), 0.0f, 20.0f, {})",
    ColrV1Gm::new("composite_mode", 0.0f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L255 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode___0_0f__0_0f_____ = "F(C(extend_mode), 0.0f, 0.0f, {})",
    ColrV1Gm::new("extend_mode", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L275 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode____0_5f__0_0f_____ = "F(C(extend_mode), -0.5f, 0.0f, {})",
    ColrV1Gm::new("extend_mode", -0.5f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L276 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode____0_5f__20_0f_____ = "F(C(extend_mode), -0.5f, 20.0f, {})",
    ColrV1Gm::new("extend_mode", -0.5f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L277 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode___0_0f__20_0f_____ = "F(C(extend_mode), 0.0f, 20.0f, {})",
    ColrV1Gm::new("extend_mode", 0.0f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L280 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_foreground_color___0_0f__0_0f_____ = "F(C(foreground_color), 0.0f, 0.0f, {})",
    ColrV1Gm::new("foreground_color", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L281 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_gradient_p2_skewed___0_0f__0_0f_____ = "F(C(gradient_p2_skewed), 0.0f, 0.0f, {})",
    ColrV1Gm::new("gradient_p2_skewed", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L282 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_gradient_stops_repeat___0_0f__0_0f_____ =
        "F(C(gradient_stops_repeat), 0.0f, 0.0f, {})",
    ColrV1Gm::new("gradient_stops_repeat", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L283 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_gradient_stops_repeat____0_5f__0_0f_____ =
        "F(C(gradient_stops_repeat), -0.5f, 0.0f, {})",
    ColrV1Gm::new("gradient_stops_repeat", -0.5f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L284 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_gradient_stops_repeat____0_5f__20_0f_____ =
        "F(C(gradient_stops_repeat), -0.5f, 20.0f, {})",
    ColrV1Gm::new("gradient_stops_repeat", -0.5f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L285 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_gradient_stops_repeat___0_0f__20_0f_____ =
        "F(C(gradient_stops_repeat), 0.0f, 20.0f, {})",
    ColrV1Gm::new("gradient_stops_repeat", 0.0f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L286 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_rotate___0_0f__0_0f_____ = "F(C(paint_rotate), 0.0f, 0.0f, {})",
    ColrV1Gm::new("paint_rotate", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L289 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_scale___0_0f__0_0f_____ = "F(C(paint_scale), 0.0f, 0.0f, {})",
    ColrV1Gm::new("paint_scale", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L293 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_skew___0_0f__0_0f_____ = "F(C(paint_skew), 0.0f, 0.0f, {})",
    ColrV1Gm::new("paint_skew", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L297 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_transform___0_0f__0_0f_____ = "F(C(paint_transform), 0.0f, 0.0f, {})",
    ColrV1Gm::new("paint_transform", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L298 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_translate___0_0f__0_0f_____ = "F(C(paint_translate), 0.0f, 0.0f, {})",
    ColrV1Gm::new("paint_translate", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L300 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep___0_0f__0_0f_____ = "F(C(sweep_varsweep), 0.0f, 0.0f, {})",
    ColrV1Gm::new("sweep_varsweep", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L301 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep____0_5f__0_0f_____ = "F(C(sweep_varsweep), -0.5f, 0.0f, {})",
    ColrV1Gm::new("sweep_varsweep", -0.5f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L302 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep____0_5f__20_0f_____ = "F(C(sweep_varsweep), -0.5f, 20.0f, {})",
    ColrV1Gm::new("sweep_varsweep", -0.5f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L303 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep___0_0f__20_0f_____ = "F(C(sweep_varsweep), 0.0f, 20.0f, {})",
    ColrV1Gm::new("sweep_varsweep", 0.0f32, 20.0f32, &[])
);

// Port of: gm/colrv1.cpp#L316 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_variable_alpha___0_0f__0_0f_____ = "F(C(variable_alpha), 0.0f, 0.0f, {})",
    ColrV1Gm::new("variable_alpha", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L319 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paintcolrglyph_cycle___0_0f__0_0f_____ =
        "F(C(paintcolrglyph_cycle), 0.0f, 0.0f, {})",
    ColrV1Gm::new("paintcolrglyph_cycle", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L320 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_coincident___0_0f__0_0f_____ = "F(C(sweep_coincident), 0.0f, 0.0f, {})",
    ColrV1Gm::new("sweep_coincident", 0.0f32, 0.0f32, &[])
);

// Port of: gm/colrv1.cpp#L321 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_glyph_nested___0_0f__0_0f_____ = "F(C(paint_glyph_nested), 0.0f, 0.0f, {})",
    ColrV1Gm::new("paint_glyph_nested", 0.0f32, 0.0f32, &[])
);
