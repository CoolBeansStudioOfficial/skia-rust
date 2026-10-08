// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/strokerect_anisotropic.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;

// Port of: gm/strokerect_anisotropic.cpp#L18-L24 (chrome/m156)
fn draw_sqooshed_rect(canvas: &Canvas, xlate: Vector, p: &Paint) {
    canvas.save();
    canvas.translate((xlate.x, xlate.y));
    canvas.scale((0.03, 2.0));
    canvas.draw_rect(Rect::new(-500.0, -10.0, 500.0, 10.0), p);
    canvas.restore();
}

// Port of: gm/strokerect_anisotropic.cpp#L26-L74 (chrome/m156)
struct StrokeRectAnisotropicGm;

impl GM for StrokeRectAnisotropicGm {
    fn name(&self) -> String {
        "strokerect_anisotropic".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(160, 160)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut aa_paint = Paint::default();
        aa_paint.set_color(Color::from_argb(255, 0, 0, 0));
        aa_paint.set_anti_alias(true);
        aa_paint.set_stroke_width(10.0);
        aa_paint.set_style(Style::Stroke);

        let mut bw_paint = Paint::default();
        bw_paint.set_color(Color::from_argb(255, 0, 0, 0));
        bw_paint.set_stroke_width(10.0);
        bw_paint.set_style(Style::Stroke);

        // The two miter columns
        draw_sqooshed_rect(canvas, Vector::new(20.0, 40.5), &aa_paint); // whole pixels
        draw_sqooshed_rect(canvas, Vector::new(20.0, 110.5), &bw_paint); // whole pixels

        draw_sqooshed_rect(canvas, Vector::new(60.5, 40.0), &aa_paint); // half pixels
        draw_sqooshed_rect(canvas, Vector::new(60.5, 110.0), &bw_paint); // half pixels

        aa_paint.set_stroke_join(Join::Bevel);
        bw_paint.set_stroke_join(Join::Bevel);

        // The two bevel columns
        draw_sqooshed_rect(canvas, Vector::new(100.0, 40.5), &aa_paint); // whole pixels
        draw_sqooshed_rect(canvas, Vector::new(100.0, 110.5), &bw_paint); // whole pixels

        draw_sqooshed_rect(canvas, Vector::new(140.5, 40.0), &aa_paint); // half pixels
        draw_sqooshed_rect(canvas, Vector::new(140.5, 110.0), &bw_paint); // half pixels
    }
}

// Port of: gm/strokerect_anisotropic.cpp#L76 (chrome/m156)
crate::def_gm!(StrokeRectAnisotropicGM, StrokeRectAnisotropicGm);
