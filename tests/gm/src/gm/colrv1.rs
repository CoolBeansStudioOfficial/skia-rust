// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colrv1.cpp (chrome/m156)
//
// Skip-matching only (docs/design/text.md §1.2): every COLRv1 test font is a resource, so
// `ToolUtils::CreateTypefaceFromResource` is `None` in the portable configuration, and each GM
// returns `DrawResult::Skip` in `onDraw`, as it did when the goldens were made. The drawing
// code is not ported, because it can never run here; a non-null typeface panics.

// Literals are kept verbatim from the C++ source (the descriptions of the test font).
#![allow(clippy::unreadable_literal)]

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
// Port of: gm/colrv1.cpp#L250 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_clipbox_0_0f_0_0f_CLIO_t_200_f = "F(C(clipbox), 0.0f, 0.0f, {{\"CLIO\"_t, 200.f}})",
    ColrV1Gm::new("clipbox", 0.0f32, 0.0f32, &[(b"CLIO", 200.0f32)])
);

// Port of: gm/colrv1.cpp#L256 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_COL1_t_0_25f_COL3_t_0_25f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"COL1\"_t, -0.25f}, {\"COL3\"_t, 0.25f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[(b"COL1", -0.25f32), (b"COL3", 0.25f32)]
    )
);

// Port of: gm/colrv1.cpp#L261 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_COL1_t_1_5f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"COL1\"_t, -1.5f}})",
    ColrV1Gm::new("extend_mode", 0.0f32, 0.0f32, &[(b"COL1", -1.5f32)])
);

// Port of: gm/colrv1.cpp#L257 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_COL1_t_0_5f_COL3_t_0_5f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"COL1\"_t, 0.5f}, {\"COL3\"_t, -0.5f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[(b"COL1", 0.5f32), (b"COL3", -0.5f32)]
    )
);

// Port of: gm/colrv1.cpp#L278 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_COL2_t_0_3f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"COL2\"_t, -0.3f}})",
    ColrV1Gm::new("extend_mode", 0.0f32, 0.0f32, &[(b"COL2", -0.3f32)])
);

// Port of: gm/colrv1.cpp#L258 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_COL3_t_0_5f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"COL3\"_t, 0.5f}})",
    ColrV1Gm::new("extend_mode", 0.0f32, 0.0f32, &[(b"COL3", 0.5f32)])
);

// Port of: gm/colrv1.cpp#L273 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_COL3_t_1_f_COL2_t_1_5f_COL1_t_2_f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"COL3\"_t, 1.f}, {\"COL2\"_t, 1.5f}, {\"COL1\"_t, 2.f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[(b"COL3", 1.0f32), (b"COL2", 1.5f32), (b"COL1", 2.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L259 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_COL3_t_1_f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"COL3\"_t, 1.f}})",
    ColrV1Gm::new("extend_mode", 0.0f32, 0.0f32, &[(b"COL3", 1.0f32)])
);

// Port of: gm/colrv1.cpp#L263 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_GRR0_t_200_f_GRR1_t_300_f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"GRR0\"_t, -200.f}, {\"GRR1\"_t, -300.f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[(b"GRR0", -200.0f32), (b"GRR1", -300.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L269 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_GRR0_t_50_f_COL3_t_2_f_COL2_t_2_f_COL1_t_0_9f = "F(C(extend_mode), 0.0f, 0.0f, {{\"GRR0\"_t, -50.f}, {\"COL3\"_t, -2.f}, {\"COL2\"_t, -2.f}, {\"COL1\"_t, -0.9f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[
            (b"GRR0", -50.0f32),
            (b"COL3", -2.0f32),
            (b"COL2", -2.0f32),
            (b"COL1", -0.9f32)
        ]
    )
);

// Port of: gm/colrv1.cpp#L271 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_GRR0_t_50_f_COL3_t_2_f_COL2_t_2_f_COL1_t_1_1f = "F(C(extend_mode), 0.0f, 0.0f, {{\"GRR0\"_t, -50.f}, {\"COL3\"_t, -2.f}, {\"COL2\"_t, -2.f}, {\"COL1\"_t, -1.1f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[
            (b"GRR0", -50.0f32),
            (b"COL3", -2.0f32),
            (b"COL2", -2.0f32),
            (b"COL1", -1.1f32)
        ]
    )
);

// Port of: gm/colrv1.cpp#L279 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_GRR0_t_430_f_GRR1_t_40_f =
        "F(C(extend_mode), 0.0f, 0.0f, {{\"GRR0\"_t, 430.f}, {\"GRR1\"_t, 40.f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[(b"GRR0", 430.0f32), (b"GRR1", 40.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L265 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_GRX0_t_1000_f_GRX1_t_1000_f_GRR0_t_1000_f_GRR1_t_900_f = "F(C(extend_mode), 0.0f, 0.0f, {{\"GRX0\"_t, -1000.f}, {\"GRX1\"_t, -1000.f}, {\"GRR0\"_t, -1000.f}, {\"GRR1\"_t, -900.f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[
            (b"GRX0", -1000.0f32),
            (b"GRX1", -1000.0f32),
            (b"GRR0", -1000.0f32),
            (b"GRR1", -900.0f32)
        ]
    )
);

// Port of: gm/colrv1.cpp#L267 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_extend_mode_0_0f_0_0f_GRX0_t_1000_f_GRX1_t_1000_f_GRR0_t_1000_f_GRR1_t_200_f = "F(C(extend_mode), 0.0f, 0.0f, {{\"GRX0\"_t, 1000.f}, {\"GRX1\"_t, -1000.f}, {\"GRR0\"_t, -1000.f}, {\"GRR1\"_t, 200.f}})",
    ColrV1Gm::new(
        "extend_mode",
        0.0f32,
        0.0f32,
        &[
            (b"GRX0", 1000.0f32),
            (b"GRX1", -1000.0f32),
            (b"GRR0", -1000.0f32),
            (b"GRR1", 200.0f32)
        ]
    )
);

// Port of: gm/colrv1.cpp#L287 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_rotate_0_0f_0_0f_ROTA_t_40_f =
        "F(C(paint_rotate), 0.0f, 0.0f, {{\"ROTA\"_t, 40.f}})",
    ColrV1Gm::new("paint_rotate", 0.0f32, 0.0f32, &[(b"ROTA", 40.0f32)])
);

// Port of: gm/colrv1.cpp#L288 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_rotate_0_0f_0_0f_ROTX_t_250_f_ROTY_t_250_f =
        "F(C(paint_rotate), 0.0f, 0.0f, {{\"ROTX\"_t, -250.f}, {\"ROTY\"_t, -250.f}})",
    ColrV1Gm::new(
        "paint_rotate",
        0.0f32,
        0.0f32,
        &[(b"ROTX", -250.0f32), (b"ROTY", -250.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L290 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_scale_0_0f_0_0f_SCOX_t_200_f_SCOY_t_200_f =
        "F(C(paint_scale), 0.0f, 0.0f, {{\"SCOX\"_t, 200.f}, {\"SCOY\"_t, 200.f}})",
    ColrV1Gm::new(
        "paint_scale",
        0.0f32,
        0.0f32,
        &[(b"SCOX", 200.0f32), (b"SCOY", 200.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L292 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_scale_0_0f_0_0f_SCSX_t_1_f_SCOY_t_1_f =
        "F(C(paint_scale), 0.0f, 0.0f, {{\"SCSX\"_t, -1.f}, {\"SCOY\"_t, -1.f}})",
    ColrV1Gm::new(
        "paint_scale",
        0.0f32,
        0.0f32,
        &[(b"SCSX", -1.0f32), (b"SCOY", -1.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L291 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_scale_0_0f_0_0f_SCSX_t_0_25f_SCOY_t_0_25f =
        "F(C(paint_scale), 0.0f, 0.0f, {{\"SCSX\"_t, 0.25f}, {\"SCOY\"_t, 0.25f}})",
    ColrV1Gm::new(
        "paint_scale",
        0.0f32,
        0.0f32,
        &[(b"SCSX", 0.25f32), (b"SCOY", 0.25f32)]
    )
);

// Port of: gm/colrv1.cpp#L296 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_skew_0_0f_0_0f_SKCX_t_200_f_SKCY_t_200_f =
        "F(C(paint_skew), 0.0f, 0.0f, {{\"SKCX\"_t, 200.f},{\"SKCY\"_t, 200.f}})",
    ColrV1Gm::new(
        "paint_skew",
        0.0f32,
        0.0f32,
        &[(b"SKCX", 200.0f32), (b"SKCY", 200.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L294 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_skew_0_0f_0_0f_SKXA_t_20_f =
        "F(C(paint_skew), 0.0f, 0.0f, {{\"SKXA\"_t, 20.f}})",
    ColrV1Gm::new("paint_skew", 0.0f32, 0.0f32, &[(b"SKXA", 20.0f32)])
);

// Port of: gm/colrv1.cpp#L295 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_skew_0_0f_0_0f_SKYA_t_20_f =
        "F(C(paint_skew), 0.0f, 0.0f, {{\"SKYA\"_t, 20.f}})",
    ColrV1Gm::new("paint_skew", 0.0f32, 0.0f32, &[(b"SKYA", 20.0f32)])
);

// Port of: gm/colrv1.cpp#L299 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_paint_translate_0_0f_0_0f_TLDX_t_100_f_TLDY_t_100_f =
        "F(C(paint_translate), 0.0f, 0.0f, {{\"TLDX\"_t, 100.f}, {\"TLDY\"_t, 100.f}})",
    ColrV1Gm::new(
        "paint_translate",
        0.0f32,
        0.0f32,
        &[(b"TLDX", 100.0f32), (b"TLDY", 100.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L309 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep_0_0f_0_0f_SWC1_t_0_25f_SWC2_t_0_083333333f_SWC3_t_0_083333333f_SWC4_t_0_25f = "F(C(sweep_varsweep), 0.0f, 0.0f, {{\"SWC1\"_t, -0.25f}, {\"SWC2\"_t, 0.083333333f}, {\"SWC3\"_t, 0.083333333f}, {\"SWC4\"_t, +0.25f}})",
    ColrV1Gm::new("sweep_varsweep", 0.0f32, 0.0f32, &[(b"SWC1", -0.25f32), (b"SWC2", 0.083333333f32), (b"SWC3", 0.083333333f32), (b"SWC4", 0.25f32)])
);

// Port of: gm/colrv1.cpp#L307 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep_0_0f_0_0f_SWPE_t_45_f =
        "F(C(sweep_varsweep), 0.0f, 0.0f, {{\"SWPE\"_t, -45.f}})",
    ColrV1Gm::new("sweep_varsweep", 0.0f32, 0.0f32, &[(b"SWPE", -45.0f32)])
);

// Port of: gm/colrv1.cpp#L306 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep_0_0f_0_0f_SWPE_t_90_f =
        "F(C(sweep_varsweep), 0.0f, 0.0f, {{\"SWPE\"_t, -90.f}})",
    ColrV1Gm::new("sweep_varsweep", 0.0f32, 0.0f32, &[(b"SWPE", -90.0f32)])
);

// Port of: gm/colrv1.cpp#L308 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep_0_0f_0_0f_SWPS_t_45_f_SWPE_t_45_f =
        "F(C(sweep_varsweep), 0.0f, 0.0f, {{\"SWPS\"_t, -45.f},{\"SWPE\"_t, 45.f}})",
    ColrV1Gm::new(
        "sweep_varsweep",
        0.0f32,
        0.0f32,
        &[(b"SWPS", -45.0f32), (b"SWPE", 45.0f32)]
    )
);

// Port of: gm/colrv1.cpp#L304 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep_0_0f_0_0f_SWPS_t_0_f =
        "F(C(sweep_varsweep), 0.0f, 0.0f, {{\"SWPS\"_t, 0.f}})",
    ColrV1Gm::new("sweep_varsweep", 0.0f32, 0.0f32, &[(b"SWPS", 0.0f32)])
);

// Port of: gm/colrv1.cpp#L305 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_sweep_varsweep_0_0f_0_0f_SWPS_t_90_f =
        "F(C(sweep_varsweep), 0.0f, 0.0f, {{\"SWPS\"_t, 90.f}})",
    ColrV1Gm::new("sweep_varsweep", 0.0f32, 0.0f32, &[(b"SWPS", 90.0f32)])
);

// Port of: gm/colrv1.cpp#L317 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_variable_alpha_0_0f_0_0f_APH1_t_0_7f =
        "F(C(variable_alpha), 0.0f, 0.0f, {{\"APH1\"_t, -0.7f}})",
    ColrV1Gm::new("variable_alpha", 0.0f32, 0.0f32, &[(b"APH1", -0.7f32)])
);

// Port of: gm/colrv1.cpp#L318 (chrome/m156)
crate::def_gm!(
    ColrV1_F_C_variable_alpha_0_0f_0_0f_APH2_t_0_7f_APH3_t_0_2f =
        "F(C(variable_alpha), 0.0f, 0.0f, {{\"APH2\"_t, -0.7f}, {\"APH3\"_t, -0.2f}})",
    ColrV1Gm::new(
        "variable_alpha",
        0.0f32,
        0.0f32,
        &[(b"APH2", -0.7f32), (b"APH3", -0.2f32)]
    )
);
