// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/aaa.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::scalar::{scalar_cos, scalar_sin};

#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn skbug_40038820_points() -> [(f32, f32); 4] {
    [
        (1.98009784, 9.0162744),
        (47.843992, 10.1922744),
        (47.804008, 11.7597256),
        (1.93990216, 10.5837256),
    ]
}

const W: i32 = 800;
const H: i32 = 800;

// Port of: gm/aaa.cpp#L20-L76 (chrome/m156)
crate::def_simple_gm!(analytic_antialias_convex, canvas, W, H, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);

    canvas.clear(Color::from(0xFFFF_FFFF));

    canvas.save();

    let mut y: f32 = 0.0;

    canvas.translate((0.0, y));
    canvas.rotate(1.0, None);
    canvas.draw_rect(skia_rust_core::rect::Rect::new(20.0, 20.0, 200.0, 200.0), &p);
    canvas.restore();

    y += 200.0;

    canvas.save();
    canvas.translate((0.0, y));
    canvas.rotate(1.0, None);
    canvas.draw_rect(skia_rust_core::rect::Rect::new(20.0, 20.0, 20.2, 200.0), &p);
    canvas.draw_rect(skia_rust_core::rect::Rect::new(20.0, 200.0, 200.0, 200.1), &p);
    canvas.draw_circle((100.0, 100.0), 30.0, &p);
    canvas.restore();

    // The following path is empty but it'll reveal bug chrome:662914
    let mut pb = PathBuilder::new();
    pb.move_to((bits_to_float(0x429b_9d5c), bits_to_float(0x4367_a041))); // 77.8073f, 231.626f
                                                                          // 77.8075f, 231.626f, 77.8074f, 231.625f, 77.8073f, 231.625f
    pb.cubic_to(
        (bits_to_float(0x429b_9d71), bits_to_float(0x4367_a022)),
        (bits_to_float(0x429b_9d64), bits_to_float(0x4367_a009)),
        (bits_to_float(0x429b_9d50), bits_to_float(0x4367_9ff2)),
    );
    pb.line_to((bits_to_float(0x429b_9d5c), bits_to_float(0x4367_a041))); // 77.8073f, 231.626f
    pb.close();
    canvas.draw_path(&pb.detach(), &p);

    // skbug.com/40038820
    y += 200.0;
    canvas.save();
    canvas.translate((0.0, y));
    p.set_anti_alias(true);
    let pts = skbug_40038820_points();
    pb.move_to(pts[0]);
    pb.line_to(pts[1]);
    pb.line_to(pts[2]);
    pb.line_to(pts[3]);
    canvas.draw_path(&pb.detach(), &p);
    canvas.restore();

    // skbug.com/40039068
    // t8888 splits the 800-high canvas into 3 pieces; the boundary is close to 266 and 534
    pb.move_to((700.0, 266.0));
    pb.line_to((710.0, 266.0));
    pb.line_to((710.0, 534.0));
    pb.line_to((700.0, 534.0));
    canvas.draw_path(&pb.detach(), &p);
});

// Port of: gm/aaa.cpp#L78-L123 (chrome/m156)
crate::def_simple_gm!(analytic_antialias_general, canvas, W, H, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);

    canvas.clear(Color::from(0xFFFF_FFFF));

    canvas.save();
    canvas.rotate(1.0, None);
    let r: f32 = 115.2;
    let c: f32 = 128.0;
    let mut builder = PathBuilder::new();
    builder.move_to((c + r, c));
    for i in 1..8 {
        #[allow(clippy::cast_precision_loss)] // int * SkScalar in C++
        let a: f32 = 2.6927937 * i as f32;
        builder.line_to((c + r * scalar_cos(a), c + r * scalar_sin(a)));
    }
    let path = builder.detach();
    canvas.draw_path(&path, &p);
    canvas.restore();

    canvas.save();
    canvas.translate((200.0, 0.0));
    canvas.rotate(1.0, None);
    p.set_style(Style::Stroke);
    p.set_stroke_width(5.0);
    canvas.draw_path(&path, &p);
    canvas.restore();

    // The following two paths test if we correctly cumulates the alpha on the middle pixel
    // column where the left rect and the right rect abut.
    p.set_style(Style::Fill);
    canvas.translate((0.0, 300.0));
    canvas.draw_path(
        &PathBuilder::new()
            .add_rect(
                skia_rust_core::rect::Rect::new(20.0, 20.0, 100.4999, 100.0),
                None,
                None,
            )
            .add_rect(
                skia_rust_core::rect::Rect::new(100.5001, 20.0, 200.0, 100.0),
                None,
                None,
            )
            .detach(),
        &p,
    );

    canvas.translate((300.0, 0.0));
    canvas.draw_path(
        &PathBuilder::new()
            .add_rect(
                skia_rust_core::rect::Rect::new(20.0, 20.0, 100.1, 100.0),
                None,
                None,
            )
            .add_rect(
                skia_rust_core::rect::Rect::new(100.9, 20.0, 200.0, 100.0),
                None,
                None,
            )
            .detach(),
        &p,
    );
});

// Port of: gm/aaa.cpp#L125-L136 (chrome/m156)
crate::def_simple_gm!(analytic_antialias_inverse, canvas, W, H, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);

    canvas.save();

    let mut path: Path = Path::circle((100.0, 100.0), 30.0, None);
    path.set_fill_type(PathFillType::InverseWinding);
    canvas.draw_path(&path, &p);
    canvas.restore();
});
