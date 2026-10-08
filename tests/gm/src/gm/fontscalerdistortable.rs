// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fontscalerdistortable.cpp (chrome/m156)
//
// Not ported: the variation sliders (`ToolUtils::VariationSliders`) only build the UI controls.
// The default portable typeface has no axes, and `fOverride` is false, so they never change the
// output. The variation position passed to `makeClone` is also not used: `TestTypeface::
// onMakeClone` returns the same typeface, and the portable font manager returns null from
// `makeFromStream`, so every cell draws the default portable typeface.

// The GM mirrors C++ arithmetic: scalar and integer conversions of small loop counts, the C++
// variable names (doAAA, doAAB) and one C++ function per GM body, so these lints do not apply.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{create_typeface_from_resource, default_portable_typeface};

const ROWS: usize = 2;
const COLS: usize = 5;

// Port of: gm/fontscalerdistortable.cpp#L35-L189 (chrome/m156), FontScalerDistortableGM
struct FontScalerDistortableGm {
    distortable: Option<Typeface>,
    typeface: [[Option<Typeface>; COLS]; ROWS],
}

impl GM for FontScalerDistortableGm {
    // Port of: gm/fontscalerdistortable.cpp#L42 (chrome/m156), getName
    fn name(&self) -> String {
        "fontscalerdistortable".to_owned()
    }

    // Port of: gm/fontscalerdistortable.cpp#L44 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(550, 700)
    }

    // Port of: gm/fontscalerdistortable.cpp#L66-L83 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // The Distortable.ttf resource is not available in the portable configuration.
        self.distortable =
            Some(create_typeface_from_resource(None, 0).unwrap_or_else(default_portable_typeface));
    }

    // Port of: gm/fontscalerdistortable.cpp#L95-L131 (chrome/m156), updateTypefaces
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(distortable) = self.distortable.clone() else {
            return;
        };
        // Row 0 clones the distortable typeface, which is the same typeface for the portable
        // manager; the other rows need a stream, which the portable manager cannot make.
        for row in 0..ROWS {
            for col in 0..COLS {
                self.typeface[row][col] = if row == 0 {
                    Some(distortable.make_clone(&FontArguments::default()))
                } else {
                    None
                };
            }
        }

        // Port of: gm/fontscalerdistortable.cpp#L143-L186 (chrome/m156), onDraw
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let mut font = Font::default();
        font.set_edging(Edging::SubpixelAntiAlias);
        let text = "abc";
        for row in 0..ROWS {
            for col in 0..COLS {
                let x = 10.0_f32;
                let mut y = 20.0_f32;
                font.set_typeface(Some(
                    self.typeface[row][col]
                        .clone()
                        .unwrap_or_else(default_portable_typeface),
                ));
                canvas.save();
                canvas.translate((30.0 + (col as f32) * 100.0, 20.0));
                canvas.rotate((col as f32) * 5.0, Some(Point::new(x, y * 10.0)));
                {
                    let mut p = Paint::default();
                    p.set_anti_alias(true);
                    let r = Rect::from_ltrb(x - 3.0, 15.0, x - 1.0, 280.0);
                    canvas.draw_rect(r, &p);
                }
                for ps in 6..=22 {
                    font.set_size(ps as f32);
                    canvas.draw_simple_text(text, TextEncoding::UTF8, (x, y), &font, &paint);
                    y += font.metrics().0;
                }
                canvas.restore();
            }
            canvas.translate((0.0, 360.0));
            font.set_subpixel(true);
            font.set_linear_metrics(true);
            font.set_baseline_snap(false);
        }
    }
}

// Port of: gm/fontscalerdistortable.cpp#L186 (chrome/m156), DEF_GM
crate::def_gm!(
    FontScalerDistortableGM,
    FontScalerDistortableGm {
        distortable: None,
        typeface: Default::default(),
    }
);
