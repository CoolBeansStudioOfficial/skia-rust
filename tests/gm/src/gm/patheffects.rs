// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/patheffects.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PathEffectBase};
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_effects::corner_path_effect;
use skia_rust_effects::dash_path_effect;
use skia_rust_effects::discrete_path_effect;
use skia_rust_effects::path_1d_path_effect::{self, Style as Path1DStyle};
use skia_rust_effects::path_2d_path_effect;

// Port of: gm/patheffects.cpp#L34-L44 (chrome/m156), compose_pe
fn compose_pe(paint: &mut Paint) {
    let corner = corner_path_effect::new(25.0).expect("a positive radius");
    let compose = match paint.path_effect() {
        Some(pe) => PathEffect::compose(pe, corner),
        None => corner,
    };
    paint.set_path_effect(compose);
}

// Port of: gm/patheffects.cpp#L46-L48 (chrome/m156), hair_pe
fn hair_pe(paint: &mut Paint) {
    paint.set_stroke_width(0.0);
}

// Port of: gm/patheffects.cpp#L50-L53 (chrome/m156), hair2_pe
fn hair2_pe(paint: &mut Paint) {
    paint.set_stroke_width(0.0);
    compose_pe(paint);
}

// Port of: gm/patheffects.cpp#L55-L58 (chrome/m156), stroke_pe
fn stroke_pe(paint: &mut Paint) {
    paint.set_stroke_width(12.0);
    compose_pe(paint);
}

// Port of: gm/patheffects.cpp#L60-L65 (chrome/m156), dash_pe
fn dash_pe(paint: &mut Paint) {
    let inter: [scalar; 4] = [20.0, 10.0, 10.0, 10.0];
    paint.set_stroke_width(12.0);
    paint.set_path_effect(dash_path_effect::new(&inter, 0.0));
    compose_pe(paint);
}

// Port of: gm/patheffects.cpp#L67-L69 (chrome/m156), gXY
const G_XY: [i32; 12] = [4, 0, 0, -4, 8, -4, 12, 0, 8, 4, 0, 4];

// Port of: gm/patheffects.cpp#L71-L75 (chrome/m156), scale
fn scale(path: &Path, scale: scalar) -> Path {
    path.make_transform(&Matrix::scale((scale, scale)))
}

// Port of: gm/patheffects.cpp#L77-L89 (chrome/m156), one_d_pe
fn one_d_pe(paint: &mut Paint) {
    let mut b = PathBuilder::new();
    b.move_to((int_to_scalar(G_XY[0]), int_to_scalar(G_XY[1])));
    let mut i = 2;
    while i < G_XY.len() {
        b.line_to((int_to_scalar(G_XY[i]), int_to_scalar(G_XY[i + 1])));
        i += 2;
    }
    b.close().offset((int_to_scalar(-6), 0.0));
    let path = scale(&b.detach(), 1.5);

    paint.set_path_effect(path_1d_path_effect::new(
        &path,
        int_to_scalar(21),
        0.0,
        Path1DStyle::Rotate,
    ));
    compose_pe(paint);
}

// Port of: gm/patheffects.cpp#L91-L92 (chrome/m156), gPE
const G_PE: [fn(&mut Paint); 5] = [hair_pe, hair2_pe, stroke_pe, dash_pe, one_d_pe];

// Port of: gm/patheffects.cpp#L94-L97 (chrome/m156), fill_pe
fn fill_pe(paint: &mut Paint) {
    paint.set_style(Style::Fill);
    paint.set_path_effect(None);
}

// Port of: gm/patheffects.cpp#L99-L101 (chrome/m156), discrete_pe
fn discrete_pe(paint: &mut Paint) {
    paint.set_path_effect(discrete_path_effect::new(10.0, 4.0, None));
}

// Port of: gm/patheffects.cpp#L103-L108 (chrome/m156), MakeTileEffect
fn make_tile_effect() -> PathEffect {
    let m = Matrix::scale((12.0, 12.0));
    path_2d_path_effect::new(&m, &Path::circle((0.0, 0.0), 5.0, None))
}

// Port of: gm/patheffects.cpp#L110-L112 (chrome/m156), tile_pe
fn tile_pe(paint: &mut Paint) {
    paint.set_path_effect(make_tile_effect());
}

// Port of: gm/patheffects.cpp#L114-L114 (chrome/m156), gPE2
const G_PE2: [fn(&mut Paint); 3] = [fill_pe, discrete_pe, tile_pe];

// Port of: gm/patheffects.cpp#L116-L171 (chrome/m156), PathEffectGM
struct PathEffectGm;

impl GM for PathEffectGm {
    // Port of: gm/patheffects.cpp#L121-L121 (chrome/m156)
    fn name(&self) -> String {
        "patheffect".to_owned()
    }

    // Port of: gm/patheffects.cpp#L123-L123 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(800, 600)
    }

    // Port of: gm/patheffects.cpp#L125-L170 (chrome/m156), onDraw
    #[allow(clippy::too_many_lines)] // one function in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);

        let pts = [
            Point::new(20.0, 20.0),
            Point::new(70.0, 120.0),
            Point::new(120.0, 30.0),
            Point::new(170.0, 80.0),
            Point::new(240.0, 50.0),
        ];
        let mut path = Path::polygon(&pts, false, None, None);

        canvas.save();
        for proc in G_PE {
            proc(&mut paint);
            canvas.draw_path(&path, &paint);
            canvas.translate((0.0, 75.0));
        }
        canvas.restore();

        let r = Rect::new(0.0, 0.0, 250.0, 120.0);
        let mut inset = r;
        inset.inset((50.0, 50.0));
        let mut b = PathBuilder::new();
        // The start indices are the defaults of the two-argument overloads (1 for the oval, 0
        // for the rect).
        b.add_oval(r, PathDirection::CW, None);
        b.add_rect(inset, PathDirection::CCW, None);
        path = b.detach();

        canvas.translate((320.0, 20.0));
        for proc in G_PE2 {
            proc(&mut paint);
            canvas.draw_path(&path, &paint);
            canvas.translate((0.0, 160.0));
        }

        let rect = IRect::from_xywh(20, 20, 60, 60);
        for proc in G_PE {
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_style(Style::Fill);
            proc(&mut p);
            canvas.draw_irect(rect, &p);
            canvas.translate((75.0, 0.0));
        }
    }
}

// Port of: gm/patheffects.cpp#L187-L241 (chrome/m156), StrokeLineInflated
// Example path effect using CTM. This "strokes" a single line segment with some stroke width,
// and then inflates the result by some number of pixels.
#[derive(Debug)]
struct StrokeLineInflated {
    radius: scalar,
    px_inflate: scalar,
}

impl PathEffectBase for StrokeLineInflated {
    // Port of: gm/patheffects.cpp#L192-L192 (chrome/m156), onNeedsCTM
    fn on_needs_ctm(&self) -> bool {
        true
    }

    // Port of: gm/patheffects.cpp#L194-L235 (chrome/m156), onFilterPath
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        ctm: &Matrix,
    ) -> bool {
        let pts = src.points();
        debug_assert_eq!(pts.len(), 2);
        let Some(inv_ctm) = ctm.invert() else {
            return false;
        };
        // For a line segment, we can just map the (scaled) normal vector to pixel-space,
        // increase its length by the desired number of pixels, and then map back to canvas space.
        let mut n = Point::new(pts[0].y - pts[1].y, pts[1].x - pts[0].x);
        if !n.set_length(self.radius) {
            return false;
        }
        let mut mapped_n = ctm.map_vector((n.x, n.y));
        if !mapped_n.set_length(mapped_n.length() + self.px_inflate) {
            return false;
        }
        n = inv_ctm.map_vector(mapped_n);
        dst.move_to(pts[0] + n);
        dst.line_to(pts[1] + n);
        dst.line_to(pts[1] - n);
        dst.line_to(pts[0] - n);
        dst.close();
        rec.set_fill_style();
        true
    }

    // Port of: gm/patheffects.cpp#L237-L237 (chrome/m156), computeFastBounds
    fn compute_fast_bounds(&self, _bounds: Option<&mut Rect>) -> bool {
        false
    }
}

// Port of: gm/patheffects.cpp#L247-L296 (chrome/m156), CTMPathEffectGM
struct CtmPathEffectGm;

impl GM for CtmPathEffectGm {
    // Port of: gm/patheffects.cpp#L249-L249 (chrome/m156)
    fn name(&self) -> String {
        "ctmpatheffect".to_owned()
    }

    // Port of: gm/patheffects.cpp#L251-L251 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(800, 600)
    }

    // Port of: gm/patheffects.cpp#L261-L295 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let stroke_width: scalar = 16.0;
        let px_inflate: scalar = 0.5;
        let path_effect = PathEffect::from_base(StrokeLineInflated {
            radius: stroke_width / 2.0,
            px_inflate,
        });
        let path = Path::line((100.0, 100.0), (200.0, 200.0));

        // Draw the inflated path, and a scaled version, in blue.
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::BLUE);
        paint.set_path_effect(path_effect);
        canvas.draw_path(&path, &paint);
        canvas.save();
        canvas.translate((150.0, 0.0));
        canvas.scale((2.5, 0.5));
        canvas.draw_path(&path, &paint);
        canvas.restore();

        // Draw the regular stroked version on top in green.
        // The inflated version should be visible underneath as a blue "border".
        paint.set_path_effect(None);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(stroke_width);
        paint.set_color(Color::GREEN);
        canvas.draw_path(&path, &paint);
        canvas.save();
        canvas.translate((150.0, 0.0));
        canvas.scale((2.5, 0.5));
        canvas.draw_path(&path, &paint);
        canvas.restore();
    }
}

// Port of: gm/patheffects.cpp#L173-L173 (chrome/m156)
crate::def_gm!(PathEffectGM_ = "PathEffectGM", PathEffectGm);
// Port of: gm/patheffects.cpp#L297-L297 (chrome/m156)
crate::def_gm!(CTMPathEffectGM_ = "CTMPathEffectGM", CtmPathEffectGm);
