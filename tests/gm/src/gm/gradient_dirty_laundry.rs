// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gradient_dirty_laundry.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/gradient_dirty_laundry.cpp#L24-L38 (chrome/m156)
struct GradData {
    count: usize,
    colors: &'static [Color4f],
    pos: Option<&'static [f32]>,
}

impl GradData {
    // Port of: gm/gradient_dirty_laundry.cpp#L35 (chrome/m156)
    fn grad(&self, tm: TileMode) -> Gradient<'_> {
        Gradient::new(
            Colors::new(
                &self.colors[..self.count],
                self.pos.map(|p| &p[..self.count]),
                tm,
                None,
            ),
            Interpolation::default(),
        )
    }
}

// Port of: gm/gradient_dirty_laundry.cpp#L40-L49 (chrome/m156)
const G_COLORS: [Color4f; 40] = {
    const FIVE: [Color4f; 5] = [
        colors::RED,
        colors::GREEN,
        colors::BLUE,
        colors::WHITE,
        colors::BLACK,
    ];
    let mut out = [colors::RED; 40];
    let mut i = 0;
    while i < 40 {
        out[i] = FIVE[i % 5];
        i += 1;
    }
    out
};

//constexpr SkScalar gPos[] = { SK_Scalar1*999/2000, SK_Scalar1*1001/2000 };

// Port of: gm/gradient_dirty_laundry.cpp#L53-L57 (chrome/m156)
static G_GRAD_DATA: [GradData; 1] = [GradData {
    count: 40,
    colors: &G_COLORS,
    pos: None,
}];

// Port of: gm/gradient_dirty_laundry.cpp#L59-L61 (chrome/m156)
fn make_linear(pts: &[Point; 2], data: &GradData, tm: TileMode) -> Option<Shader> {
    shaders::linear_gradient((pts[0], pts[1]), &data.grad(tm), None)
}

// Port of: gm/gradient_dirty_laundry.cpp#L63-L69 (chrome/m156)
fn make_radial(pts: &[Point; 2], data: &GradData, tm: TileMode) -> Option<Shader> {
    let pt = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    shaders::radial_gradient((pt, pt.x), &data.grad(tm), None)
}

// Port of: gm/gradient_dirty_laundry.cpp#L71-L77 (chrome/m156)
fn make_sweep(pts: &[Point; 2], data: &GradData, tm: TileMode) -> Option<Shader> {
    let pt = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    shaders::sweep_gradient(pt, (0.0, 360.0), &data.grad(tm), None)
}

type GradMaker = fn(&[Point; 2], &GradData, TileMode) -> Option<Shader>;

// Port of: gm/gradient_dirty_laundry.cpp#L81-L87 (chrome/m156)
const G_GRAD_MAKERS: [GradMaker; 3] = [make_linear, make_radial, make_sweep];

// Port of: gm/gradient_dirty_laundry.cpp#L91-L125 (chrome/m156)
struct GradientsGm;

impl GM for GradientsGm {
    fn name(&self) -> String {
        "gradient_dirty_laundry".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 615)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFDD_DDDD)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
        let tm = TileMode::Clamp;
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        canvas.translate((20.0, 20.0));
        for data in &G_GRAD_DATA {
            canvas.save();
            for maker in &G_GRAD_MAKERS {
                paint.set_shader(maker(&pts, data, tm));
                canvas.draw_rect(r, &paint);
                canvas.translate((0.0, 120.0));
            }
            canvas.restore();
            canvas.translate((120.0, 0.0));
        }
    }
}

// Port of: gm/gradient_dirty_laundry.cpp#L131 (chrome/m156)
crate::def_gm!(GradientsGM, GradientsGm);
