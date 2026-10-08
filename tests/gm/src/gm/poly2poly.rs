// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/poly2poly.cpp (chrome/m156)

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
use skia_rust_core::font::Font;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utils::text_utils::{Align, draw};
use skia_rust_tools::font_tool_utils::{create_typeface_from_resource, default_portable_typeface};

// Port of: gm/poly2poly.cpp#L13-L18 (chrome/m156), doDraw
fn do_draw(canvas: &Canvas, font: &Font, paint: &mut Paint, isrc: &[i32], idst: &[i32]) {
    const D: f32 = 64.0;
    let src: Vec<Point> = isrc
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Point::new(p[0] as f32, p[1] as f32))
        .collect();
    let dst: Vec<Point> = idst
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Point::new(p[0] as f32, p[1] as f32))
        .collect();

    canvas.save();
    if let Some(mx) = Matrix::poly_to_poly(&src, &dst) {
        canvas.concat(&mx);
    }

    paint.set_color(Color::from_argb(0xFF, 0x88, 0x88, 0x88)); // SK_ColorGRAY
    paint.set_style(Style::Stroke);
    canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, D, D), paint);
    canvas.draw_line((0.0, 0.0), (D, D), paint);
    canvas.draw_line((0.0, D), (D, 0.0), paint);

    let (_, fm) = font.metrics();
    paint.set_color(Color::RED);
    paint.set_style(Style::Fill);
    let x = D / 2.0;
    // C++ computes (a + b) / 2 as written; `midpoint` could round differently.
    #[allow(clippy::manual_midpoint)]
    let y = D / 2.0 - (fm.ascent + fm.descent) / 2.0;
    let glyph_id: GlyphId = 3; // X
    draw(
        canvas,
        &glyph_id.to_ne_bytes(),
        TextEncoding::GlyphId,
        x,
        y,
        font,
        paint,
        Align::Center,
    );
    canvas.restore();
}

// Port of: gm/poly2poly.cpp#L20-L94 (chrome/m156), Poly2PolyGM
struct Poly2PolyGm {
    em_face: Option<Typeface>,
}

impl GM for Poly2PolyGm {
    // Port of: gm/poly2poly.cpp#L22 (chrome/m156), getName
    fn name(&self) -> String {
        "poly2poly".to_owned()
    }

    // Port of: gm/poly2poly.cpp#L23 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(835, 840)
    }

    // Port of: gm/poly2poly.cpp#L70-L76 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // The Em.ttf resource is not available in the portable configuration, so this is the
        // default typeface, as in every golden.
        self.em_face =
            Some(create_typeface_from_resource(None, 0).unwrap_or_else(default_portable_typeface));
    }

    // Port of: gm/poly2poly.cpp#L78-L108 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(em_face) = self.em_face.clone() else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_stroke_width(4.0);
        let font = Font::from_size(em_face, 40.0);

        canvas.save();
        canvas.translate((10.0, 10.0));
        // translate (1 point)
        let src1 = [0, 0];
        let dst1 = [5, 5];
        do_draw(canvas, &font, &mut paint, &src1, &dst1);
        canvas.restore();

        canvas.save();
        canvas.translate((160.0, 10.0));
        // rotate/uniform-scale (2 points)
        let src2 = [32, 32, 64, 32];
        let dst2 = [32, 32, 64, 48];
        do_draw(canvas, &font, &mut paint, &src2, &dst2);
        canvas.restore();

        canvas.save();
        canvas.translate((10.0, 110.0));
        // rotate/skew (3 points)
        let src3 = [0, 0, 64, 0, 0, 64];
        let dst3 = [0, 0, 96, 0, 24, 64];
        do_draw(canvas, &font, &mut paint, &src3, &dst3);
        canvas.restore();

        canvas.save();
        canvas.translate((160.0, 110.0));
        // perspective (4 points)
        let src4 = [0, 0, 64, 0, 64, 64, 0, 64];
        let dst4 = [0, 0, 96, 0, 64, 96, 0, 64];
        do_draw(canvas, &font, &mut paint, &src4, &dst4);
        canvas.restore();
    }
}

// Port of: gm/poly2poly.cpp#L122 (chrome/m156), DEF_GM
crate::def_gm!(Poly2PolyGM, Poly2PolyGm { em_face: None });
