// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/convexpaths.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

const K_LENGTH: i32 = 100;
const K_PTS_PER_SIDE: i32 = 1 << 12;

// Port of: gm/convexpaths.cpp#L33-L371 (chrome/m156)
struct ConvexPathsGM {
    paths: Vec<Path>,
    done_once: bool,
}

impl ConvexPathsGM {
    fn new() -> Self {
        Self {
            paths: Vec::new(),
            done_once: false,
        }
    }

    // Port of: gm/convexpaths.cpp#L43-L331 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
    fn make_paths(&mut self) {
        if self.done_once {
            return;
        }
        self.done_once = true;

        let fp = &mut self.paths;
        let mut b = PathBuilder::new();
        fp.push(
            b.move_to((0.0, 0.0))
                .quad_to((50.0, 100.0), (0.0, 100.0))
                .line_to((0.0, 0.0))
                .detach(),
        );

        fp.push(
            b.move_to((0.0, 50.0))
                .quad_to((50.0, 0.0), (100.0, 50.0))
                .quad_to((50.0, 100.0), (0.0, 50.0))
                .detach(),
        );

        fp.push(Path::rect(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PathDirection::CW,
        ));
        fp.push(Path::rect(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PathDirection::CCW,
        ));
        fp.push(Path::circle((50.0, 50.0), 50.0, PathDirection::CW));
        fp.push(Path::oval(
            Rect::from_xywh(0.0, 0.0, 50.0, 100.0),
            PathDirection::CW,
        ));
        fp.push(Path::oval(
            Rect::from_xywh(0.0, 0.0, 100.0, 5.0),
            PathDirection::CCW,
        ));
        fp.push(Path::oval(
            Rect::from_xywh(0.0, 0.0, 1.0, 100.0),
            PathDirection::CCW,
        ));
        fp.push(Path::rrect(
            RRect::new_rect_xy(Rect::new(0.0, 0.0, 100.0, 100.0), 40.0, 20.0),
            PathDirection::CW,
        ));

        // large number of points
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        let length = K_LENGTH as f32;
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        let side = |i: i32| length * i as f32 / K_PTS_PER_SIDE as f32;

        b.move_to((0.0, 0.0));
        for i in 1..K_PTS_PER_SIDE {
            // skip the first point due to moveTo.
            b.line_to((side(i), 0.0));
        }
        for i in 0..K_PTS_PER_SIDE {
            b.line_to((length, side(i)));
        }
        for i in (1..=K_PTS_PER_SIDE).rev() {
            b.line_to((side(i), length));
        }
        for i in (1..=K_PTS_PER_SIDE).rev() {
            b.line_to((0.0, side(i)));
        }
        fp.push(b.detach());

        // shallow diagonals
        fp.push(Path::polygon(
            &[
                Point::new(0.0, 0.0),
                Point::new(100.0, 1.0),
                Point::new(98.0, 100.0),
                Point::new(3.0, 96.0),
            ],
            false,
            None,
            None,
        ));

        fp.push(
            b.arc_to(Rect::from_xywh(0.0, 0.0, 50.0, 100.0), 25.0, 130.0, false)
                .detach(),
        );

        // cubics
        fp.push(b.cubic_to((1.0, 1.0), (10.0, 90.0), (0.0, 100.0)).detach());
        fp.push(
            b.cubic_to((100.0, 50.0), (20.0, 100.0), (0.0, 0.0))
                .detach(),
        );

        // path that has a cubic with a repeated first control point and
        // a repeated last control point.
        fp.push(
            b.move_to((10.0, 10.0))
                .cubic_to((10.0, 10.0), (10.0, 0.0), (20.0, 0.0))
                .line_to((40.0, 0.0))
                .cubic_to((40.0, 0.0), (50.0, 0.0), (50.0, 10.0))
                .detach(),
        );

        // path that has two cubics with repeated middle control points.
        fp.push(
            b.move_to((10.0, 10.0))
                .cubic_to((10.0, 0.0), (10.0, 0.0), (20.0, 0.0))
                .line_to((40.0, 0.0))
                .cubic_to((50.0, 0.0), (50.0, 0.0), (50.0, 10.0))
                .detach(),
        );

        // cubic where last three points are almost a line
        fp.push(
            b.move_to((0.0, 228.0 / 8.0))
                .cubic_to(
                    (628.0 / 8.0, 82.0 / 8.0),
                    (1255.0 / 8.0, 141.0 / 8.0),
                    (1883.0 / 8.0, 202.0 / 8.0),
                )
                .detach(),
        );

        // flat cubic where the at end point tangents both point outward.
        fp.push(
            b.move_to((10.0, 0.0))
                .cubic_to((0.0, 1.0), (30.0, 1.0), (20.0, 0.0))
                .detach(),
        );

        // flat cubic where initial tangent is in, end tangent out
        fp.push(
            b.move_to((0.0, 0.0))
                .cubic_to((10.0, 1.0), (30.0, 1.0), (20.0, 0.0))
                .detach(),
        );

        // flat cubic where initial tangent is out, end tangent in
        fp.push(
            b.move_to((10.0, 0.0))
                .cubic_to((0.0, 1.0), (20.0, 1.0), (30.0, 0.0))
                .detach(),
        );

        // triangle where one edge is a degenerate quad
        fp.push(
            b.move_to((8.59375, 45.0))
                .quad_to((16.992_187_5, 45.0), (31.25, 45.0))
                .line_to((100.0, 100.0))
                .line_to((8.59375, 45.0))
                .detach(),
        );

        // triangle where one edge is a quad with a repeated point
        fp.push(
            b.move_to((0.0, 25.0))
                .line_to((50.0, 0.0))
                .quad_to((50.0, 50.0), (50.0, 50.0))
                .detach(),
        );

        // triangle where one edge is a cubic with a 2x repeated point
        fp.push(
            b.move_to((0.0, 25.0))
                .line_to((50.0, 0.0))
                .cubic_to((50.0, 0.0), (50.0, 50.0), (50.0, 50.0))
                .detach(),
        );

        // triangle where one edge is a quad with a nearly repeated point
        fp.push(
            b.move_to((0.0, 25.0))
                .line_to((50.0, 0.0))
                .quad_to((50.0, 49.95), (50.0, 50.0))
                .detach(),
        );

        // triangle where one edge is a cubic with a 3x nearly repeated point
        fp.push(
            b.move_to((0.0, 25.0))
                .line_to((50.0, 0.0))
                .cubic_to((50.0, 49.95), (50.0, 49.97), (50.0, 50.0))
                .detach(),
        );

        // triangle where there is a point degenerate cubic at one corner
        fp.push(
            b.move_to((0.0, 25.0))
                .line_to((50.0, 0.0))
                .line_to((50.0, 50.0))
                .cubic_to((50.0, 50.0), (50.0, 50.0), (50.0, 50.0))
                .detach(),
        );

        // point line
        fp.push(Path::line((50.0, 50.0), (50.0, 50.0)));

        // point quad
        fp.push(
            b.move_to((50.0, 50.0))
                .quad_to((50.0, 50.0), (50.0, 50.0))
                .detach(),
        );

        // point cubic
        fp.push(
            b.move_to((50.0, 50.0))
                .cubic_to((50.0, 50.0), (50.0, 50.0), (50.0, 50.0))
                .detach(),
        );

        // moveTo only paths
        fp.push(
            b.move_to((0.0, 0.0))
                .move_to((0.0, 0.0))
                .move_to((1.0, 1.0))
                .move_to((1.0, 1.0))
                .move_to((10.0, 10.0))
                .detach(),
        );

        fp.push(b.move_to((0.0, 0.0)).move_to((0.0, 0.0)).detach());

        // line degenerate
        fp.push(b.line_to((100.0, 100.0)).detach());
        fp.push(b.quad_to((100.0, 100.0), (0.0, 0.0)).detach());
        fp.push(b.quad_to((100.0, 100.0), (50.0, 50.0)).detach());
        fp.push(b.quad_to((50.0, 50.0), (100.0, 100.0)).detach());
        fp.push(b.cubic_to((0.0, 0.0), (0.0, 0.0), (100.0, 100.0)).detach());

        // skbug.com/40040207
        let mut m = Matrix::new_identity();
        m.set_all(0.1, 0.0, -1.0, 0.0, 0.115_207, -2.64977, 0.0, 0.0, 1.0);
        fp.push(
            b.move_to((16.875, 192.594))
                .cubic_to((45.625, 192.594), (74.375, 192.594), (103.125, 192.594))
                .cubic_to((88.75, 167.708), (74.375, 142.823), (60.0, 117.938))
                .cubic_to((45.625, 142.823), (31.25, 167.708), (16.875, 192.594))
                .close()
                .transform(&m)
                .detach(),
        );

        // small circle. This is listed last so that it has device coords far
        // from the origin (small area relative to x,y values).
        fp.push(Path::circle((0.0, 0.0), 1.2, None));
    }
}

impl GM for ConvexPathsGM {
    fn name(&self) -> String {
        "convexpaths".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1200, 1100)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFF00_0000)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        self.make_paths();

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let mut rand = Random::default();
        canvas.translate((20.0, 20.0));

        // As we've added more paths this has gotten pretty big. Scale the whole thing down.
        canvas.scale((2.0 / 3.0, 2.0 / 3.0));

        for (i, path) in self.paths.iter().enumerate() {
            canvas.save();
            // position the path, and make it at off-integer coords.
            #[allow(clippy::cast_precision_loss)] // int arithmetic then conversion, as in C++
            canvas.translate((
                200.0 * (i % 5) as f32 + 1.0 / 10.0,
                200.0 * (i / 5) as f32 + 9.0 / 10.0,
            ));
            let mut color = rand.next_u();
            color |= 0xff00_0000;
            paint.set_color(Color::from(color));
            canvas.draw_path(path, &paint);
            canvas.restore();
        }
    }
}

// Port of: gm/convexpaths.cpp#L373 (chrome/m156)
crate::def_gm!(ConvexPathsGM_ = "ConvexPathsGM", ConvexPathsGM::new());
