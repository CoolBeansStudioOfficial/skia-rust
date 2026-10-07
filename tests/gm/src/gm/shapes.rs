// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/shapes.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Vector;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::{RRect, Type as RRectType};

/*
 * This is the base class for two GMs that cover various corner cases with primitive Skia shapes
 * (zero radius, near-zero radius, inner shape overlap, etc.) It uses an xfermode of darken to help
 * double-blended and/or dropped pixels stand out.
 */
// Port of: gm/shapes.cpp#L27-L82 (chrome/m156)
struct ShapesGM {
    name: String,
    antialias: bool,
    paint: Paint,
    shapes: Vec<RRect>,
    rotations: Vec<f32>,
    simple_shape_count: usize,
    inner: bool,
}

impl ShapesGM {
    // Port of: gm/shapes.cpp#L29-L33 (chrome/m156)
    fn new(name: &str, antialias: bool, inner: bool) -> Self {
        let mut name = name.to_owned();
        if !antialias {
            name.push_str("_bw");
        }
        Self {
            name,
            antialias,
            paint: Paint::default(),
            shapes: Vec::new(),
            rotations: Vec::new(),
            simple_shape_count: 0,
            inner,
        }
    }

    // Port of: gm/shapes.cpp#L82-L113 (chrome/m156)
    fn draw_simple_shapes(&self, canvas: &Canvas) {
        let mut rand = Random::new(2);
        for i in 0..self.shapes.len() {
            let mut paint = self.paint.clone();
            paint.set_color(Color::from(rand.next_u() & !0x0080_8080));
            paint.set_alpha_f(0.5); // Use alpha to detect double blends.
            let shape = &self.shapes[i];
            canvas.save();
            canvas.rotate(self.rotations[i], None);
            match shape.get_type() {
                RRectType::Rect => {
                    canvas.draw_rect(shape.rect(), &paint);
                }
                RRectType::Oval => {
                    canvas.draw_oval(shape.rect(), &paint);
                }
                _ => {
                    canvas.draw_rrect(shape, &paint);
                }
            }
            canvas.restore();
        }
    }

    // Port of: gm/shapes.cpp#L122-L173 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // never: sizes are small ints
    fn draw_inner_shapes(&self, canvas: &Canvas) {
        let mut rand = Random::default();
        for i in 0..self.shapes.len() {
            let outer = &self.shapes[i];
            let inner = &self.shapes[(i * 7 + 11) % self.simple_shape_count];
            let mut s: f32 = 0.95
                * (outer.rect().width() / inner.rect().width())
                    .min(outer.rect().height() / inner.rect().height());
            let mut inner_xform = Matrix::new_identity();
            let mut dx = (rand.next_f() - 0.5) * (outer.rect().width() - s * inner.rect().width());
            let mut dy =
                (rand.next_f() - 0.5) * (outer.rect().height() - s * inner.rect().height());
            // Fixup inner rects so they don't reach outside the outer rect.
            match i {
                0 => {
                    s *= 0.85;
                }
                8 => {
                    s *= 0.4;
                    dx = 0.0;
                    dy = 0.0;
                }
                5 => {
                    s *= 0.75;
                    dx = 0.0;
                    dy = 0.0;
                }
                6 => {
                    s *= 0.65;
                    dx = -5.0;
                    dy = 10.0;
                }
                _ => {}
            }
            inner_xform.set_translate((outer.rect().center_x() + dx, outer.rect().center_y() + dy));
            if s < 1.0 {
                inner_xform.pre_scale((s, s), None);
            }
            inner_xform.pre_translate((-inner.rect().center_x(), -inner.rect().center_y()));
            let xformed_inner = inner.transform(&inner_xform).unwrap_or_default();
            let mut paint = self.paint.clone();
            paint.set_color(Color::from(rand.next_u() & !0x0080_8080));
            paint.set_alpha_f(0.5); // Use alpha to detect double blends.
            canvas.save();
            canvas.rotate(self.rotations[i], None);
            canvas.draw_drrect(outer, xformed_inner, &paint);
            canvas.restore();
        }
    }
}

impl GM for ShapesGM {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn size(&mut self) -> ISize {
        ISize::new(500, 500)
    }

    // Port of: gm/shapes.cpp#L40-L68 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let mut shape = RRect::default();
        shape.set_oval(Rect::from_xywh(-5.0, 25.0, 200.0, 100.0));
        self.shapes.push(shape);
        self.rotations.push(21.0);

        let mut shape = RRect::default();
        shape.set_rect(Rect::from_xywh(95.0, 75.0, 125.0, 100.0));
        self.shapes.push(shape);
        self.rotations.push(94.0);

        let mut shape = RRect::default();
        shape.set_rect_xy(Rect::from_xywh(0.0, 75.0, 150.0, 100.0), 1e-5, 1e-5);
        self.shapes.push(shape);
        self.rotations.push(132.0);

        let mut shape = RRect::default();
        shape.set_rect_xy(Rect::from_xywh(15.0, -20.0, 100.0, 100.0), 20.0, 15.0);
        self.shapes.push(shape);
        self.rotations.push(282.0);

        self.simple_shape_count = self.shapes.len();

        let mut shape = RRect::default();
        shape.set_nine_patch(
            Rect::from_xywh(140.0, -50.0, 90.0, 110.0),
            10.0,
            5.0,
            25.0,
            35.0,
        );
        self.shapes.push(shape);
        self.rotations.push(0.0);

        let mut shape = RRect::default();
        shape.set_nine_patch(
            Rect::from_xywh(160.0, -60.0, 60.0, 90.0),
            10.0,
            60.0,
            50.0,
            30.0,
        );
        self.shapes.push(shape);
        self.rotations.push(-35.0);

        let mut shape = RRect::default();
        shape.set_nine_patch(
            Rect::from_xywh(220.0, -120.0, 60.0, 90.0),
            1.0,
            89.0,
            59.0,
            1.0,
        );
        self.shapes.push(shape);
        self.rotations.push(65.0);

        let radii: [Vector; 4] = [
            Vector::new(4.0, 6.0),
            Vector::new(12.0, 8.0),
            Vector::new(24.0, 16.0),
            Vector::new(32.0, 48.0),
        ];
        let mut shape = RRect::default();
        shape.set_rect_radii(Rect::from_xywh(150.0, -129.0, 80.0, 160.0), &radii);
        self.shapes.push(shape);
        self.rotations.push(265.0);

        let radii2: [Vector; 4] = [
            Vector::new(0.0, 0.0),
            Vector::new(80.0, 60.0),
            Vector::new(0.0, 0.0),
            Vector::new(80.0, 60.0),
        ];
        let mut shape = RRect::default();
        shape.set_rect_radii(Rect::from_xywh(180.0, -30.0, 80.0, 60.0), &radii2);
        self.shapes.push(shape);
        self.rotations.push(295.0);

        self.paint.set_anti_alias(self.antialias);
    }

    // Port of: gm/shapes.cpp#L70-L77 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::WHITE);

        canvas.save();
        #[allow(clippy::cast_precision_loss)] // int / 2.f in C++
        canvas.translate((
            canvas.image_info().width() as f32 / 2.0,
            canvas.image_info().height() as f32 / 2.0,
        ));
        if self.inner {
            self.draw_inner_shapes(canvas);
        } else {
            self.draw_simple_shapes(canvas);
        }
        canvas.restore();
    }
}

// Port of: gm/shapes.cpp#L186-L189 (chrome/m156)
crate::def_gm!(
    SimpleShapesGM_true = "SimpleShapesGM(true)",
    ShapesGM::new("simpleshapes", true, false)
);
crate::def_gm!(
    SimpleShapesGM_false = "SimpleShapesGM(false)",
    ShapesGM::new("simpleshapes", false, false)
);
crate::def_gm!(
    InnerShapesGM_true = "InnerShapesGM(true)",
    ShapesGM::new("innershapes", true, true)
);
crate::def_gm!(
    InnerShapesGM_false = "InnerShapesGM(false)",
    ShapesGM::new("innershapes", false, true)
);
