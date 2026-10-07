// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/strokerect.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::{AutoCanvasRestore, PointMode};
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::rect::Rect;

// Port of: gm/strokerect.cpp#L27 (chrome/m156)
const STROKE_WIDTH: f32 = 20.0;

// Port of: gm/strokerect.cpp#L29-L48 (chrome/m156)
fn draw_path(canvas: &Canvas, path: &Path, rect: &Rect, join: Join, do_fill: bool) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(if do_fill {
        Style::StrokeAndFill
    } else {
        Style::Stroke
    });

    paint.set_color(Color::GRAY);
    paint.set_stroke_width(STROKE_WIDTH);
    paint.set_stroke_join(join);
    canvas.draw_rect(rect, &paint);

    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);
    paint.set_color(Color::RED);
    canvas.draw_path(path, &paint);

    paint.set_stroke_width(3.0);
    paint.set_stroke_join(Join::Miter);
    canvas.draw_points(PointMode::Points, path.points(), &paint);
}

// Test calling SkStroker for rectangles. Cases to cover:
//
// geometry: normal, small (smaller than stroke-width), empty, inverted
// joint-type for the corners
// Port of: gm/strokerect.cpp#L57-L121 (chrome/m156)
struct StrokeRectGm;

impl GM for StrokeRectGm {
    fn name(&self) -> String {
        "strokerect".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1400, 740)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::WHITE, None);
        canvas.translate((STROKE_WIDTH * 3.0 / 2.0, STROKE_WIDTH * 3.0 / 2.0));

        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(STROKE_WIDTH);

        let g_joins = [Join::Miter, Join::Round, Join::Bevel];

        const W: f32 = 80.0;
        const H: f32 = 80.0;
        let g_rects = [
            Rect::new(0.0, 0.0, W, H),
            Rect::new(W, 0.0, 0.0, H),
            Rect::new(0.0, H, W, 0.0),
            Rect::new(0.0, 0.0, STROKE_WIDTH, H),
            Rect::new(0.0, 0.0, W, STROKE_WIDTH),
            Rect::new(0.0, 0.0, STROKE_WIDTH / 2.0, STROKE_WIDTH / 2.0),
            Rect::new(0.0, 0.0, W, 0.0),
            Rect::new(0.0, 0.0, 0.0, H),
            Rect::new(0.0, 0.0, 0.0, 0.0),
            Rect::new(0.0, 0.0, W, f32::EPSILON),
            Rect::new(0.0, 0.0, f32::EPSILON, H),
            Rect::new(0.0, 0.0, f32::EPSILON, f32::EPSILON),
        ];

        for do_fill in 0..=1 {
            for join in g_joins {
                paint.set_stroke_join(join);

                let acr = AutoCanvasRestore::guard(canvas, true);
                for r in &g_rects {
                    let (fill_path, _) = fill_path_with_paint_to_path(&Path::rect(r, None), &paint);
                    draw_path(canvas, &fill_path, r, join, do_fill != 0);

                    canvas.translate((W + 2.0 * STROKE_WIDTH, 0.0));
                }
                acr.restore();
                canvas.translate((0.0, H + 2.0 * STROKE_WIDTH));
            }
            paint.set_style(Style::StrokeAndFill);
        }
    }
}
crate::def_gm!(StrokeRectGM, StrokeRectGm);

// Exercise rect-stroking (which is specialized from paths) when the resulting stroke-width is
// non-square. See https://bugs.chromium.org/p/skia/issues/detail?id=5408
// Port of: gm/strokerect.cpp#L123-L133 (chrome/m156)
crate::def_simple_gm!(strokerect_anisotropic_5408, canvas, 200, 50, {
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    p.set_stroke_width(6.0);

    canvas.scale((10.0, 1.0));
    let r = Rect::from_xywh(5.0, 20.0, 10.0, 10.0);
    canvas.draw_rect(r, &p);
});
