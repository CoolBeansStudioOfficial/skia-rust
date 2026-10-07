// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/cubicpaths.cpp (chrome/m156)

// Float literals are copied verbatim from the C++ source.
#![allow(clippy::excessive_precision)]

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// skbug.com/40032398 shows that this cubic, when slightly clipped, creates big
// (incorrect) changes to its control points.
// Port of: gm/cubicpaths.cpp#L31-L61 (chrome/m156)
struct ClippedCubicGm;

impl GM for ClippedCubicGm {
    fn name(&self) -> String {
        "clippedcubic".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1240, 390)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let path = PathBuilder::new()
            .move_to((0.0, 0.0))
            .cubic_to((140.0, 150.0), (40.0, 10.0), (170.0, 150.0))
            .detach();

        let paint = Paint::default();
        let bounds = *path.bounds();

        let mut dy: f32 = -1.0;
        while dy <= 1.0 {
            canvas.save();
            let mut dx: f32 = -1.0;
            while dx <= 1.0 {
                canvas.save();
                canvas.clip_rect(bounds, None, None);
                canvas.translate((dx, dy));
                canvas.draw_path(&path, &paint);
                canvas.restore();

                canvas.translate((bounds.width(), 0.0));
                dx += 1.0;
            }
            canvas.restore();
            canvas.translate((0.0, bounds.height()));
            dy += 1.0;
        }
    }
}

// Port of: gm/cubicpaths.cpp#L63-L124 (chrome/m156)
#[derive(Default)]
struct ClippedCubic2Gm {
    path: Path,
    flipped: Path,
}

impl ClippedCubic2Gm {
    // Port of: gm/cubicpaths.cpp#L92-L102 (chrome/m156)
    fn draw_one(canvas: &Canvas, path: &Path, clip: &Rect) {
        let mut frame_paint = Paint::default();
        let fill_paint = Paint::default();
        frame_paint.set_style(Style::Stroke);
        canvas.draw_rect(clip, &frame_paint);
        canvas.draw_path(path, &frame_paint);
        canvas.save();
        canvas.clip_rect(clip, None, None);
        canvas.draw_path(path, &fill_paint);
        canvas.restore();
    }
}

impl GM for ClippedCubic2Gm {
    fn name(&self) -> String {
        "clippedcubic2".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1240, 390)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.save();
        canvas.translate((-2.0, 120.0));
        Self::draw_one(canvas, &self.path, &Rect::from_ltrb(0.0, 0.0, 80.0, 150.0));
        canvas.translate((0.0, 170.0));
        Self::draw_one(canvas, &self.path, &Rect::from_ltrb(0.0, 0.0, 80.0, 100.0));
        canvas.translate((0.0, 170.0));
        Self::draw_one(canvas, &self.path, &Rect::from_ltrb(0.0, 0.0, 30.0, 150.0));
        canvas.translate((0.0, 170.0));
        Self::draw_one(canvas, &self.path, &Rect::from_ltrb(0.0, 0.0, 10.0, 150.0));
        canvas.restore();
        canvas.save();
        canvas.translate((20.0, -2.0));
        Self::draw_one(canvas, &self.flipped, &Rect::from_ltrb(0.0, 0.0, 150.0, 80.0));
        canvas.translate((170.0, 0.0));
        Self::draw_one(canvas, &self.flipped, &Rect::from_ltrb(0.0, 0.0, 100.0, 80.0));
        canvas.translate((170.0, 0.0));
        Self::draw_one(canvas, &self.flipped, &Rect::from_ltrb(0.0, 0.0, 150.0, 30.0));
        canvas.translate((170.0, 0.0));
        Self::draw_one(canvas, &self.flipped, &Rect::from_ltrb(0.0, 0.0, 150.0, 10.0));
        canvas.restore();
    }

    fn on_once_before_draw(&mut self) {
        let mut builder = PathBuilder::new();
        builder.move_to((69.7030518991886, 0.0));
        builder.cubic_to(
            (69.7030518991886, 21.831149999999997),
            (58.08369508178456, 43.66448333333333),
            (34.8449814469765, 65.5),
        );
        builder.cubic_to(
            (11.608591683531916, 87.33115),
            (-0.010765133872116195, 109.16448333333332),
            (-0.013089005235602302, 131.0),
        );
        builder.close();
        self.path = builder.detach();

        let mut matrix = Matrix::new_identity();
        matrix.reset();
        matrix.set_scale_x(0.0);
        matrix.set_scale_y(0.0);
        matrix.set_skew_x(1.0);
        matrix.set_skew_y(1.0);
        self.flipped = self.path.make_transform(&matrix);
    }
}

// Port of: gm/cubicpaths.cpp#L499-L511 (chrome/m156)
crate::def_simple_gm!(bug5099, canvas, 50, 50, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(10.0);

    let path = PathBuilder::new()
        .move_to((6.0, 27.0))
        .cubic_to((31.5, 1.5), (3.5, 4.5), (29.0, 29.0))
        .detach();
    canvas.draw_path(&path, &p);
});

// Port of: gm/cubicpaths.cpp#L513-L540 (chrome/m156)
crate::def_simple_gm!(bug6083, canvas, 100, 50, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(15.0);
    canvas.translate((-500.0, -130.0));

    let mut builder = PathBuilder::new();
    builder.move_to((500.988, 155.200)).line_to((526.109, 155.200));

    let _path = builder.snapshot();
    let p1 = Point::new(526.109, 155.200);
    let mut p2 = Point::new(525.968, 212.968);
    let p3 = Point::new(526.109, 241.840);

    builder.cubic_to(p1, p2, p3);
    canvas.draw_path(&builder.detach(), &p);
    canvas.translate((50.0, 0.0));

    p2.set(525.968, 213.172);
    builder.move_to((500.988, 155.200));
    builder.line_to((526.109, 155.200));
    builder.cubic_to(p1, p2, p3);
    canvas.draw_path(&builder.detach(), &p);
});

//////////////////////////////////////////////////////////////////////////////

// Port of: gm/cubicpaths.cpp#L545-L546 (chrome/m156)
crate::def_gm!(ClippedCubicGM, ClippedCubicGm);
crate::def_gm!(ClippedCubic2GM, ClippedCubic2Gm::default());
