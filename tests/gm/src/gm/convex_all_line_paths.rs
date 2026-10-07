// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/convex_all_line_paths.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{degrees_to_radians, scalar_cos, scalar_sin};

// Port of: gm/convex_all_line_paths.cpp#L19-L31 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int -> float arithmetic as in C++
fn create_ngon(n: i32, pts: &mut [Point], width: f32, height: f32) {
    let angle_step: f32 = 360.0 / n as f32;
    let mut angle: f32 = 0.0;
    if (n % 2) == 1 {
        angle = angle_step / 2.0;
    }

    for pt in pts.iter_mut().take(n as usize) {
        pt.x = -scalar_sin(degrees_to_radians(angle)) * width;
        pt.y = scalar_cos(degrees_to_radians(angle)) * height;
        angle += angle_step;
    }
}

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

// Port of: gm/convex_all_line_paths.cpp#L33-L150 (chrome/m156)
fn g_points() -> Vec<Vec<Point>> {
    vec![
        // narrow rect
        vec![p(-1.5, -50.0), p(1.5, -50.0), p(1.5, 50.0), p(-1.5, 50.0)],
        // narrow rect on an angle
        vec![p(-50.0, -49.0), p(-49.0, -50.0), p(50.0, 49.0), p(49.0, 50.0)],
        // trap - narrow on top - wide on bottom
        vec![p(-10.0, -50.0), p(10.0, -50.0), p(50.0, 50.0), p(-50.0, 50.0)],
        // wide skewed rect
        vec![p(-50.0, -50.0), p(0.0, -50.0), p(50.0, 50.0), p(0.0, 50.0)],
        // thin rect with colinear-ish lines
        vec![
            p(-6.0, -50.0),
            p(4.0, -50.0),
            p(5.0, -25.0), // remove if collinear diagonal points are not concave
            p(6.0, 0.0),
            p(5.0, 25.0), // remove if collinear diagonal points are not concave
            p(4.0, 50.0),
            p(-4.0, 50.0),
        ],
        // degenerate
        vec![p(-0.025, -0.025), p(0.025, -0.025), p(0.025, 0.025), p(-0.025, 0.025)],
        // Triangle in which the first point should fuse with last
        vec![p(-20.0, -13.0), p(-20.0, -13.05), p(20.0, -13.0), p(20.0, 27.0)],
        // thin rect with colinear lines
        vec![
            p(-10.0, -50.0),
            p(10.0, -50.0),
            p(10.0, -25.0),
            p(10.0, 0.0),
            p(10.0, 25.0),
            p(10.0, 50.0),
            p(-10.0, 50.0),
        ],
        // capped teardrop
        vec![
            p(50.00, 50.00),
            p(0.00, 50.00),
            p(-15.45, 47.55),
            p(-29.39, 40.45),
            p(-40.45, 29.39),
            p(-47.55, 15.45),
            p(-50.00, 0.00),
            p(-47.55, -15.45),
            p(-40.45, -29.39),
            p(-29.39, -40.45),
            p(-15.45, -47.55),
            p(0.00, -50.00),
            p(50.00, -50.00),
        ],
        // teardrop
        vec![
            p(4.39, 40.45),
            p(-9.55, 47.55),
            p(-25.00, 50.00),
            p(-40.45, 47.55),
            p(-54.39, 40.45),
            p(-65.45, 29.39),
            p(-72.55, 15.45),
            p(-75.00, 0.00),
            p(-72.55, -15.45),
            p(-65.45, -29.39),
            p(-54.39, -40.45),
            p(-40.45, -47.55),
            p(-25.0, -50.0),
            p(-9.55, -47.55),
            p(4.39, -40.45),
            p(75.00, 0.00),
        ],
        // clipped triangle
        vec![
            p(-10.0, -50.0),
            p(10.0, -50.0),
            p(50.0, 31.0),
            p(40.0, 50.0),
            p(-40.0, 50.0),
            p(-50.0, 31.0),
        ],
    ]
}

const K_STROKE_WIDTH: i32 = 10;
const K_NUM_PATHS: i32 = 20;
const K_MAX_PATH_HEIGHT: i32 = 100;
const K_GM_WIDTH: i32 = 512;
const K_GM_HEIGHT: i32 = 512;

// This GM is intended to exercise Ganesh's handling of convex line-only
// paths
// Port of: gm/convex_all_line_paths.cpp#L154-L411 (chrome/m156)
struct ConvexLineOnlyPathsGM {
    do_stroke_and_fill: bool,
    points: Vec<Vec<Point>>,
}

impl ConvexLineOnlyPathsGM {
    fn new(do_stroke_and_fill: bool) -> Self {
        Self {
            do_stroke_and_fill,
            points: g_points(),
        }
    }

    // Port of: gm/convex_all_line_paths.cpp#L173-L249 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar and int / 2 in C++
    fn get_path(&self, index: i32, dir: PathDirection) -> Path {
        let data: Vec<Point>;
        let points: &[Point];
        let num_pts: i32;
        if (index as usize) < self.points.len() {
            // manually specified
            points = &self.points[index as usize];
            num_pts = points.len() as i32;
        } else {
            // procedurally generated
            let mut width: f32 = (K_MAX_PATH_HEIGHT / 2) as f32;
            let height: f32 = (K_MAX_PATH_HEIGHT / 2) as f32;
            match index as usize - self.points.len() {
                0 => num_pts = 3,
                1 => num_pts = 4,
                2 => num_pts = 5,
                3 => {
                    // squashed pentagon
                    num_pts = 5;
                    width = (K_MAX_PATH_HEIGHT / 5) as f32;
                }
                4 => num_pts = 6,
                5 => num_pts = 8,
                6 => {
                    // squashed octogon
                    num_pts = 8;
                    width = (K_MAX_PATH_HEIGHT / 5) as f32;
                }
                7 => num_pts = 20,
                8 => num_pts = 100,
                _ => num_pts = 3,
            }

            let mut d = vec![Point::default(); num_pts as usize];

            create_ngon(num_pts, &mut d, width, height);
            data = d;
            points = &data;
        }

        let mut builder = PathBuilder::new();

        if PathDirection::CW == dir {
            builder.move_to(points[0]);
            for i in 1..num_pts as usize {
                builder.line_to(points[i]);
            }
        } else {
            builder.move_to(points[num_pts as usize - 1]);
            for i in (0..=(num_pts as usize - 2)).rev() {
                builder.line_to(points[i]);
            }
        }

        builder.close();
        builder.detach()
    }

    // Draw a single path several times, shrinking it, flipping its direction
    // and changing its start vertex each time.
    // Port of: gm/convex_all_line_paths.cpp#L253-L303 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn draw_path(&self, canvas: &Canvas, index: i32, offset: &mut Point) {
        let center: Point;
        {
            let path = self.get_path(index, PathDirection::CW);
            if offset.x + path.bounds().width() > K_GM_WIDTH as f32 {
                offset.x = 0.0;
                offset.y += K_MAX_PATH_HEIGHT as f32;
                if self.do_stroke_and_fill {
                    offset.x += K_STROKE_WIDTH as f32 / 2.0;
                    offset.y += K_STROKE_WIDTH as f32 / 2.0;
                }
            }
            center = Point::new(offset.x + (path.bounds().width() / 2.0), offset.y);
            offset.x += path.bounds().width();
            if self.do_stroke_and_fill {
                offset.x += K_STROKE_WIDTH as f32;
            }
        }

        let colors = [Color::BLACK, Color::WHITE];
        let dirs = [PathDirection::CW, PathDirection::CCW];
        let scales: [f32; 7] = [1.0, 0.75, 0.5, 0.25, 0.1, 0.01, 0.001];
        let joins = [Join::Round, Join::Bevel, Join::Miter];

        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        for (i, scale) in scales.iter().enumerate() {
            let path = self.get_path(index, dirs[i % 2]);
            if self.do_stroke_and_fill {
                paint.set_style(Style::StrokeAndFill);
                paint.set_stroke_join(joins[i % 3]);
                paint.set_stroke_width(K_STROKE_WIDTH as f32);
            }

            canvas.save();
            canvas.translate((center.x, center.y));
            canvas.scale((*scale, *scale));
            paint.set_color(colors[i % 2]);
            canvas.draw_path(&path, &paint);
            canvas.restore();
        }
    }
}

impl GM for ConvexLineOnlyPathsGM {
    fn name(&self) -> String {
        if self.do_stroke_and_fill {
            return "convex-lineonly-paths-stroke-and-fill".to_owned();
        }
        "convex-lineonly-paths".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(K_GM_WIDTH, K_GM_HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFFF_FFFF)
    }

    // Port of: gm/convex_all_line_paths.cpp#L305-L407 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    #[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn on_draw(&mut self, canvas: &Canvas) {
        // the right edge of the last drawn path
        let mut offset = Point::new(0.0, K_MAX_PATH_HEIGHT as f32 / 2.0);
        if self.do_stroke_and_fill {
            offset.x += K_STROKE_WIDTH as f32 / 2.0;
            offset.y += K_STROKE_WIDTH as f32 / 2.0;
        }

        for i in 0..K_NUM_PATHS {
            self.draw_path(canvas, i, &mut offset);
        }

        {
            // Repro for crbug.com/472723 (Missing AA on portions of graphic with GPU rasterization)

            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            if self.do_stroke_and_fill {
                paint.set_style(Style::StrokeAndFill);
                paint.set_stroke_join(Join::Miter);
                paint.set_stroke_width(K_STROKE_WIDTH as f32);
            }

            let p1 = Path::polygon(
                &[
                    p(60.8522949, 364.671021),
                    p(59.4380493, 364.671021),
                    p(385.414276, 690.647217),
                    p(386.121399, 689.940125),
                ],
                false,
                None,
                None,
            );
            canvas.save();
            canvas.translate((356.0, 50.0));
            canvas.draw_path(&p1, &paint);
            canvas.restore();

            // Repro for crbug.com/869172 (SVG path incorrectly simplified when using GPU
            // Rasterization). This will only draw anything in the stroke-and-fill version.
            let p2 = Path::polygon(
                &[
                    p(10.0, 0.0),
                    p(38.0, 0.0),
                    p(66.0, 0.0),
                    p(94.0, 0.0),
                    p(122.0, 0.0),
                    p(150.0, 0.0),
                    p(150.0, 0.0),
                    p(122.0, 0.0),
                    p(94.0, 0.0),
                    p(66.0, 0.0),
                    p(38.0, 0.0),
                    p(10.0, 0.0),
                ],
                true,
                None,
                None,
            );
            canvas.save();
            canvas.translate((0.0, 500.0));
            canvas.draw_path(&p2, &paint);
            canvas.restore();

            // Repro for crbug.com/856137. This path previously caused GrAAConvexTessellator to turn
            // inset rings into outsets when adjacent bisector angles converged outside the previous
            // ring due to accumulated error.
            let p3 = Path::polygon(
                &[
                    p(1184.96, 982.557),
                    p(1183.71, 982.865),
                    p(1180.99, 982.734),
                    p(1178.5, 981.541),
                    p(1176.35, 979.367),
                    p(1178.94, 938.854),
                    p(1181.35, 936.038),
                    p(1183.96, 934.117),
                    p(1186.67, 933.195),
                    p(1189.36, 933.342),
                    p(1191.58, 934.38),
                ],
                true,
                PathFillType::EvenOdd,
                None,
            );
            canvas.save();
            let mut m = Matrix::new_identity();
            m.set_all(0.0893210843, 0.0, 79.1197586, 0.0, 0.0893210843, 300.0, 0.0, 0.0, 1.0);
            canvas.concat(&m);
            canvas.draw_path(&p3, &paint);
            canvas.restore();
        }
    }
}

// Port of: gm/convex_all_line_paths.cpp#L413-L414 (chrome/m156)
crate::def_gm!(
    ConvexLineOnlyPathsGM_false = "ConvexLineOnlyPathsGM(false)",
    ConvexLineOnlyPathsGM::new(false)
);
crate::def_gm!(
    ConvexLineOnlyPathsGM_true = "ConvexLineOnlyPathsGM(true)",
    ConvexLineOnlyPathsGM::new(true)
);
