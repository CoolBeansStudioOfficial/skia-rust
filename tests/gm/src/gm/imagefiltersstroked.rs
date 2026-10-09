// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersstroked.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::color::Color4f;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar_invert;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, drop_shadow, matrix_transform, offset};

const RESIZE_FACTOR_X: f32 = 2.0;
const RESIZE_FACTOR_Y: f32 = 5.0;

type DrawProc = fn(&Canvas, Rect, &Paint);

// Port of: gm/imagefiltersstroked.cpp#L19-L23 (chrome/m156), draw_circle
fn draw_circle(canvas: &Canvas, r: Rect, paint: &Paint) {
    canvas.draw_circle((r.center_x(), r.center_y()), r.width() * 2.0 / 5.0, paint);
}

// Port of: gm/imagefiltersstroked.cpp#L24-L26 (chrome/m156), draw_line
fn draw_line(canvas: &Canvas, r: Rect, paint: &Paint) {
    canvas.draw_line((r.left(), r.bottom()), (r.right(), r.top()), paint);
}

// Port of: gm/imagefiltersstroked.cpp#L27-L29 (chrome/m156), draw_rect
fn draw_rect(canvas: &Canvas, r: Rect, paint: &Paint) {
    canvas.draw_rect(r, paint);
}

// Port of: gm/imagefiltersstroked.cpp#L14-L78 (chrome/m156), ImageFiltersStrokedGM
struct ImageFiltersStrokedGm;

impl GM for ImageFiltersStrokedGm {
    fn name(&self) -> String {
        "imagefiltersstroked".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(860, 500)
    }

    fn bg_color(&self) -> Color {
        Color::from(0x0000_0000)
    }

    // Port of: gm/imagefiltersstroked.cpp#L31-L76 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let draw_proc: [DrawProc; 3] = [draw_line, draw_rect, draw_circle];
        canvas.clear(Color::BLACK);
        let mut resize_matrix = Matrix::new_identity();
        resize_matrix.set_scale((RESIZE_FACTOR_X, RESIZE_FACTOR_Y), None);
        let filters: [Option<ImageFilter>; 4] = [
            blur(5.0, 5.0, TileMode::Decal, None, None),
            drop_shadow(
                (10.0, 10.0),
                (3.0, 3.0),
                Color4f::from(Color::GREEN),
                None,
                None,
                None,
            ),
            offset((-16.0, 32.0), None, None),
            matrix_transform(&resize_matrix, SamplingOptions::default(), None),
        ];

        let r = Rect::from_wh(64.0, 64.0);
        let margin: f32 = 32.0;
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        paint.set_anti_alias(true);
        paint.set_stroke_width(10.0);
        paint.set_style(Style::Stroke);
        for proc_fn in draw_proc {
            canvas.translate((0.0, margin));
            canvas.save();
            for (j, filter) in filters.iter().enumerate() {
                canvas.translate((margin, 0.0));
                canvas.save();
                if j == 2 {
                    canvas.translate((16.0, -32.0));
                } else if j == 3 {
                    canvas.scale((
                        scalar_invert(RESIZE_FACTOR_X),
                        scalar_invert(RESIZE_FACTOR_Y),
                    ));
                }
                paint.set_image_filter(filter.clone());
                proc_fn(canvas, r, &paint);
                canvas.restore();
                canvas.translate((r.width() + margin, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, r.height()));
        }
    }
}

// Port of: gm/imagefiltersstroked.cpp#L108 (chrome/m156), DEF_GM(return new ImageFiltersStrokedGM;)
crate::def_gm!(ImageFiltersStrokedGM, ImageFiltersStrokedGm);
