// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/sharedcorners.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{scalar_cos, scalar_sin, SCALAR_PI};

const K_PAD_SIZE: i32 = 20;
const K_BOX_SIZE: i32 = 100;
// Port of: gm/sharedcorners.cpp#L21 (chrome/m156)
const K_JITTERS: [Point; 3] = [
    Point::new(0.0, 0.0),
    Point::new(0.5, 0.5),
    Point::new(2.0 / 3.0, 1.0 / 3.0),
];

// Tests various corners of different angles falling on the same pixel, particularly to ensure
// analytic AA is working properly.
// Port of: gm/sharedcorners.cpp#L25-L163 (chrome/m156)
struct SharedCornersGM {
    wire_frame_paint: Paint,
    fill_paint: Paint,
}

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

impl SharedCornersGM {
    fn new() -> Self {
        Self {
            wire_frame_paint: Paint::default(),
            fill_paint: Paint::default(),
        }
    }

    // Port of: gm/sharedcorners.cpp#L87-L121 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int -> SkScalar arithmetic as in C++
    fn draw_triangle_boxes(&self, canvas: &Canvas, points: &[Point], triangles: &[[usize; 3]]) {
        let mut builder = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
        builder.set_is_volatile(true);
        for triangle in triangles {
            builder.move_to(points[triangle[0]]);
            builder.line_to(points[triangle[1]]);
            builder.line_to(points[triangle[2]]);
            builder.close();
        }
        let bounds = builder.compute_bounds();
        let scale: f32 = K_BOX_SIZE as f32 / bounds.height().max(bounds.width());
        builder.transform(&Matrix::scale((scale, scale)));
        let mut path = builder.detach();

        self.draw_row(canvas, &path);
        canvas.translate((0.0, (K_BOX_SIZE + K_PAD_SIZE) as f32));

        let mut rot = Matrix::new_identity();
        rot.set_rotate(
            45.0,
            Point::new(path.bounds().center_x(), path.bounds().center_y()),
        );
        path = path.make_transform(&rot);
        self.draw_row(canvas, &path);
        canvas.translate((0.0, (K_BOX_SIZE + K_PAD_SIZE) as f32));

        rot.set_rotate(
            -45.0 - 69.381_11,
            Point::new(path.bounds().center_x(), path.bounds().center_y()),
        );
        path = path.make_transform(&rot);
        self.draw_row(canvas, &path);
        canvas.translate((0.0, (K_BOX_SIZE + K_PAD_SIZE) as f32));
    }

    // Port of: gm/sharedcorners.cpp#L123-L141 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int -> SkScalar arithmetic as in C++
    fn draw_row(&self, canvas: &Canvas, path: &Path) {
        canvas.save();
        let bounds = path.bounds();
        canvas.translate((
            (K_BOX_SIZE as f32 - bounds.width()) / 2.0 - bounds.left,
            (K_BOX_SIZE as f32 - bounds.height()) / 2.0 - bounds.top,
        ));

        canvas.draw_path(path, &self.wire_frame_paint);
        canvas.translate(((K_BOX_SIZE + K_PAD_SIZE) as f32, 0.0));

        for jitter in K_JITTERS {
            {
                canvas.save();
                canvas.translate((jitter.x, jitter.y));
                canvas.draw_path(path, &self.fill_paint);
                canvas.restore();
            }
            canvas.translate(((K_BOX_SIZE + K_PAD_SIZE) as f32, 0.0));
        }
        canvas.restore();
    }
}

impl GM for SharedCornersGM {
    fn name(&self) -> String {
        "sharedcorners".to_owned()
    }

    fn size(&mut self) -> ISize {
        const NUM_ROWS: i32 = 3 * 2;
        const NUM_COLS: i32 = (1 + K_JITTERS.len() as i32) * 2;
        ISize::new(
            NUM_COLS * (K_BOX_SIZE + K_PAD_SIZE) + K_PAD_SIZE,
            NUM_ROWS * (K_BOX_SIZE + K_PAD_SIZE) + K_PAD_SIZE,
        )
    }

    fn bg_color(&self) -> Color {
        crate::tool_utils::color_to_565(0xFF1A_65D7)
    }

    // Port of: gm/sharedcorners.cpp#L40-L46 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.fill_paint.set_color(Color::WHITE);
        self.fill_paint.set_anti_alias(true);

        self.wire_frame_paint = self.fill_paint.clone();
        self.wire_frame_paint.set_style(Style::Stroke);
    }

    // Port of: gm/sharedcorners.cpp#L48-L85 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // int -> SkScalar arithmetic as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((K_PAD_SIZE as f32, K_PAD_SIZE as f32));
        canvas.save();

        // Adjacent rects.
        self.draw_triangle_boxes(
            canvas,
            &[
                p(0.0, 0.0),
                p(40.0, 0.0),
                p(80.0, 0.0),
                p(120.0, 0.0),
                p(0.0, 20.0),
                p(40.0, 20.0),
                p(80.0, 20.0),
                p(120.0, 20.0),
                p(40.0, 40.0),
                p(80.0, 40.0),
                p(40.0, 60.0),
                p(80.0, 60.0),
            ],
            &[
                [0, 1, 4],
                [1, 5, 4],
                [5, 1, 6],
                [1, 2, 6],
                [2, 3, 6],
                [3, 7, 6],
                [8, 5, 9],
                [5, 6, 9],
                [10, 8, 11],
                [8, 9, 11],
            ],
        );

        // Obtuse angles.
        self.draw_triangle_boxes(
            canvas,
            &[
                p(0.0, 0.0),
                p(10.0, 0.0),
                p(20.0, 0.0),
                p(0.0, 2.0),
                p(20.0, 2.0),
                p(10.0, 4.0),
                p(0.0, 6.0),
                p(20.0, 6.0),
                p(0.0, 8.0),
                p(10.0, 8.0),
                p(20.0, 8.0),
            ],
            &[
                [3, 1, 4],
                [4, 5, 3],
                [6, 5, 7],
                [7, 9, 6],
                [0, 1, 3],
                [1, 2, 4],
                [3, 5, 6],
                [5, 4, 7],
                [6, 9, 8],
                [9, 7, 10],
            ],
        );

        canvas.restore();
        canvas.translate((((K_BOX_SIZE + K_PAD_SIZE) * 4) as f32, 0.0));

        // Right angles.
        self.draw_triangle_boxes(
            canvas,
            &[p(0.0, 0.0), p(-1.0, 0.0), p(0.0, -1.0), p(1.0, 0.0), p(0.0, 1.0)],
            &[[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]],
        );

        // Acute angles.
        let mut rand = Random::default();
        let mut pts: Vec<Point> = Vec::new();
        let mut indices: Vec<[usize; 3]> = Vec::new();
        let mut theta: f32 = 0.0;
        pts.push(p(0.0, 0.0));
        while theta < 2.0 * SCALAR_PI {
            pts.push(p(scalar_cos(theta), scalar_sin(theta)));
            if pts.len() > 2 {
                indices.push([0, pts.len() - 2, pts.len() - 1]);
            }
            theta += rand.next_range_f(0.0, SCALAR_PI / 3.0);
        }
        indices.push([0, pts.len() - 1, 1]);
        self.draw_triangle_boxes(canvas, &pts, &indices);
    }
}

// Port of: gm/sharedcorners.cpp#L165 (chrome/m156)
crate::def_gm!(SharedCornersGM_ = "SharedCornersGM", SharedCornersGM::new());
