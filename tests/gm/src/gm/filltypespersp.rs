// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/filltypespersp.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::color::Color4f;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/filltypespersp.cpp#L13-L31 (chrome/m156)
struct FillTypePerspGm {
    path: Path,
}

impl FillTypePerspGm {
    fn new() -> Self {
        Self { path: Path::new() }
    }

    // Port of: gm/filltypespersp.cpp#L16-L23 (chrome/m156)
    fn make_path(&mut self) {
        if self.path.is_empty() {
            let radius: f32 = 45.0;
            self.path = PathBuilder::new()
                .add_circle((50.0, 50.0), radius, PathDirection::CW)
                .add_circle((100.0, 100.0), radius, PathDirection::CW)
                .detach();
        }
    }

    // Port of: gm/filltypespersp.cpp#L27-L40 (chrome/m156)
    fn show_path(
        &mut self,
        canvas: &Canvas,
        x: i32,
        y: i32,
        ft: PathFillType,
        scale: f32,
        paint: &Paint,
    ) {
        let r = Rect::from_ltrb(0.0, 0.0, 150.0, 150.0);
        canvas.save();
        canvas.translate((x as f32, y as f32));
        canvas.clip_rect(r, None, None);
        canvas.draw_color(Color::WHITE, None);
        self.path.set_fill_type(ft);
        canvas.translate((r.center_x(), r.center_y()));
        canvas.scale((scale, scale));
        canvas.translate((-r.center_x(), -r.center_y()));
        canvas.draw_path(&self.path, paint);
        canvas.restore();
    }

    // Port of: gm/filltypespersp.cpp#L41-L60 (chrome/m156)
    fn show_four(&mut self, canvas: &Canvas, scale: f32, aa: bool) {
        let mut paint = Paint::default();
        let center = Point::new(100.0, 100.0);
        let colors = [
            Color4f::new(0.0, 0.0, 1.0, 1.0),
            Color4f::new(1.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 1.0, 0.0, 1.0),
        ];
        let pos: [f32; 3] = [0.0, 0.5, 1.0];
        paint.set_shader(shaders::radial_gradient(
            (center, 100.0),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        paint.set_anti_alias(aa);
        self.show_path(canvas, 0, 0, PathFillType::Winding, scale, &paint);
        self.show_path(canvas, 200, 0, PathFillType::EvenOdd, scale, &paint);
        self.show_path(canvas, 0, 200, PathFillType::InverseWinding, scale, &paint);
        self.show_path(
            canvas,
            200,
            200,
            PathFillType::InverseEvenOdd,
            scale,
            &paint,
        );
    }
}

impl GM for FillTypePerspGm {
    fn name(&self) -> String {
        "filltypespersp".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(835, 840)
    }

    // Port of: gm/filltypespersp.cpp#L61-L99 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        self.make_path();
        let mut bkgnrd = Paint::default();
        let center = Point::new(100.0, 100.0);
        let colors = [
            Color4f::new(0.0, 0.0, 0.0, 1.0),
            Color4f::new(0.0, 1.0, 1.0, 1.0),
            Color4f::new(1.0, 1.0, 0.0, 1.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
        ];
        let pos: [f32; 4] = [0.0, 0.5 / 2.0, 3.0 * 0.5 / 2.0, 1.0];
        bkgnrd.set_shader(shaders::radial_gradient(
            (center, 1000.0),
            &Gradient::new(
                Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        canvas.save();
        canvas.translate((100.0, 100.0));
        let mut mat = Matrix::new_identity();
        mat.set_persp_y(1.0 / 1000.0);
        canvas.concat(&mat);
        canvas.draw_paint(&bkgnrd);
        canvas.restore();

        let mut persp = Matrix::new_identity();
        persp.set_persp_x(-1.0 / 1800.0);
        persp.set_persp_y(1.0 / 500.0);
        canvas.concat(&persp);
        canvas.translate((20.0, 20.0));
        let scale: f32 = 5.0 / 4.0;
        self.show_four(canvas, 1.0, false);
        canvas.translate((450.0, 0.0));
        self.show_four(canvas, scale, false);
        canvas.translate((-450.0, 450.0));
        self.show_four(canvas, 1.0, true);
        canvas.translate((450.0, 0.0));
        self.show_four(canvas, scale, true);
    }
}

crate::def_gm!(FillTypePerspGM, FillTypePerspGm::new());
