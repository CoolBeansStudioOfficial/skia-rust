// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/daa.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/daa.cpp#L21 (chrome/m156), K
const K: scalar = 49.0;

// Port of: gm/daa.cpp#L23-L129 (chrome/m156), DEF_SIMPLE_GM(daa, ...)

// The body of DEF_SIMPLE_GM(daa): its five blocks, in order.
#[allow(clippy::too_many_lines)] // the C++ body is long for the same reason
fn draw_daa(canvas: &Canvas) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let font: Font = default_portable_font();
    let label = |canvas: &Canvas, text: &str, paint: &Paint| {
        canvas.draw_simple_text(
            text.as_bytes(),
            TextEncoding::UTF8,
            (K * 1.5, K * 0.5),
            &font,
            paint,
        );
    };
    let square = Rect::from_ltrb(0.0, 0.0, K, K);

    {
        paint.set_color(Color::BLACK);
        label(
            canvas,
            "Should be a green square with no red showing through.",
            &paint,
        );
        paint.set_color(Color::RED);
        canvas.draw_rect(square, &paint);
        let tri1 = [(0.0, 0.0), (K, K), (0.0, K), (0.0, 0.0)];
        let tri2 = [(0.0, 0.0), (K, K), (K, 0.0), (0.0, 0.0)];
        let path: Path = PathBuilder::new()
            .add_polygon(&points(&tri1), false)
            .add_polygon(&points(&tri2), false)
            .detach();
        paint.set_color(Color::GREEN);
        canvas.draw_path(&path, &paint);
    }
    canvas.translate((0.0, K));
    {
        paint.set_color(Color::BLACK);
        label(
            canvas,
            "Adjacent rects, two draws.  Blue then green, no red?",
            &paint,
        );
        paint.set_color(Color::RED);
        canvas.draw_rect(square, &paint);
        {
            let path = polygon(&[(0.0, 0.0), (0.0, K), (K * 0.5, K), (K * 0.5, 0.0)]);
            paint.set_color(Color::BLUE);
            canvas.draw_path(&path, &paint);
        }
        {
            let path = polygon(&[(K * 0.5, 0.0), (K * 0.5, K), (K, K), (K, 0.0)]);
            paint.set_color(Color::GREEN);
            canvas.draw_path(&path, &paint);
        }
    }
    canvas.translate((0.0, K));
    {
        paint.set_color(Color::BLACK);
        label(
            canvas,
            "Adjacent rects, wound together.  All green?",
            &paint,
        );
        paint.set_color(Color::RED);
        canvas.draw_rect(square, &paint);
        {
            let path = PathBuilder::new()
                .add_polygon(
                    &points(&[(0.0, 0.0), (0.0, K), (K * 0.5, K), (K * 0.5, 0.0)]),
                    false,
                )
                .add_polygon(
                    &points(&[(K * 0.5, 0.0), (K * 0.5, K), (K, K), (K, 0.0)]),
                    false,
                )
                .detach();
            paint.set_color(Color::GREEN);
            canvas.draw_path(&path, &paint);
        }
    }
    canvas.translate((0.0, K));
    {
        paint.set_color(Color::BLACK);
        label(
            canvas,
            "Adjacent rects, wound opposite.  All green?",
            &paint,
        );
        paint.set_color(Color::RED);
        canvas.draw_rect(square, &paint);
        {
            let path = PathBuilder::new()
                .add_polygon(
                    &points(&[(0.0, 0.0), (0.0, K), (K * 0.5, K), (K * 0.5, 0.0)]),
                    false,
                )
                .add_polygon(
                    &points(&[(K * 0.5, 0.0), (K, 0.0), (K, K), (K * 0.5, K)]),
                    false,
                )
                .detach();
            paint.set_color(Color::GREEN);
            canvas.draw_path(&path, &paint);
        }
    }
    canvas.translate((0.0, K));
    {
        paint.set_color(Color::BLACK);
        label(canvas, "One poly, wound opposite.  All green?", &paint);
        paint.set_color(Color::RED);
        canvas.draw_rect(square, &paint);
        let path = polygon(&[
            (K * 0.5, 0.0),
            (0.0, 0.0),
            (0.0, K),
            (K * 0.5, K),
            (K * 0.5, 0.0),
            (K, 0.0),
            (K, K),
            (K * 0.5, K),
        ]);
        paint.set_color(Color::GREEN);
        canvas.draw_path(&path, &paint);
    }
}

crate::def_simple_gm!(daa, canvas, 49 + 350, 5 * 49, {
    draw_daa(canvas);
});

// An open polygon of the GM's `SkPoint` array (`SkPath::Polygon(pts, false)`).
fn polygon(pts: &[(scalar, scalar)]) -> Path {
    Path::polygon(&points(pts), false, None, None)
}

// The `SkPoint` arrays of the GM as points.
fn points(pts: &[(scalar, scalar)]) -> Vec<Point> {
    pts.iter().map(|&(x, y)| Point::new(x, y)).collect()
}
