// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/circulararcs.cpp (chrome/m156)

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
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_PI;
use skia_rust_effects::dash_path_effect;

// Port of: gm/circulararcs.cpp#L25-L31 (chrome/m156)
const STARTS: [f32; 8] = [0.0, 10.0, 30.0, 45.0, 90.0, 165.0, 180.0, 270.0];
const SWEEPS: [f32; 8] = [1.0, 45.0, 90.0, 130.0, 180.0, 184.0, 300.0, 355.0];
const DIAMETER: f32 = 40.0;
const RECT: Rect = Rect::new(0.0, 0.0, DIAMETER, DIAMETER);
const W: i32 = 1000;
const H: i32 = 1000;

// Port of: gm/circulararcs.cpp#L33-L81 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar, kW / 2.f
fn draw_arcs(canvas: &Canvas, configure_style: impl Fn(&mut Paint)) {
    // Draws grid of arcs with different start/sweep angles in red and their complement arcs in
    // blue.
    let draw_grid = |x: f32, y: f32, use_center: bool, aa: bool| {
        const PAD: f32 = 20.0;
        let mut p0 = Paint::default();
        p0.set_color(Color::RED);
        p0.set_anti_alias(aa);
        // Set a reasonable stroke width that configureStyle can override.
        p0.set_stroke_width(15.0);
        let mut p1 = p0.clone();
        p1.set_color(Color::BLUE);
        // Use alpha so we see magenta on overlap between arc and its complement.
        p0.set_alpha(100);
        p1.set_alpha(100);
        configure_style(&mut p0);
        configure_style(&mut p1);

        canvas.save();
        canvas.translate((PAD + x, PAD + y));
        for start in STARTS {
            canvas.save();
            for sweep in SWEEPS {
                canvas.draw_arc(RECT, start, sweep, use_center, &p0);
                canvas.draw_arc(RECT, start, -(360.0 - sweep), use_center, &p1);
                canvas.translate((RECT.width() + PAD, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, RECT.height() + PAD));
        }
        canvas.restore();
    };
    // Draw a grids for combo of enabling/disabling aa and using center.
    const GRID_W: f32 = W as f32 / 2.0;
    const GRID_H: f32 = H as f32 / 2.0;
    draw_grid(0.0, 0.0, false, false);
    draw_grid(GRID_W, 0.0, true, false);
    draw_grid(0.0, GRID_H, false, true);
    draw_grid(GRID_W, GRID_H, true, true);
    // Draw separators between the grids.
    let mut line_paint = Paint::default();
    line_paint.set_anti_alias(true);
    line_paint.set_color(Color::BLACK);
    canvas.draw_line((GRID_W, 0.0), (GRID_W, H as f32), &line_paint);
    canvas.draw_line((0.0, GRID_H), (W as f32, GRID_H), &line_paint);
}

// Port of: gm/circulararcs.cpp#L83-L88 (chrome/m156)
// `DEF_ARC_GM(fill)`: the manifest id is the macro argument, the GM is named `circular_arcs_fill`.
crate::def_simple_gm_bg_name!(fill, canvas, W, H, Color::WHITE, "circular_arcs_fill", {
    let set_fill = |p: &mut Paint| {
        p.set_stroke(false);
    };
    draw_arcs(canvas, set_fill);
});

// Port of: gm/circulararcs.cpp#L90-L98 (chrome/m156)
// `DEF_ARC_GM(hairline)`: the manifest id is the macro argument, the GM is named `circular_arcs_hairline`.
crate::def_simple_gm_bg_name!(
    hairline,
    canvas,
    W,
    H,
    Color::WHITE,
    "circular_arcs_hairline",
    {
        let set_hairline = |p: &mut Paint| {
            p.set_stroke(true);
            p.set_stroke_width(0.0);
        };
        draw_arcs(canvas, set_hairline);
    }
);

// Port of: gm/circulararcs.cpp#L100-L107 (chrome/m156)
// `DEF_ARC_GM(stroke_butt)`: the manifest id is the macro argument, the GM is named `circular_arcs_stroke_butt`.
crate::def_simple_gm_bg_name!(
    stroke_butt,
    canvas,
    W,
    H,
    Color::WHITE,
    "circular_arcs_stroke_butt",
    {
        let set_stroke = |p: &mut Paint| {
            p.set_stroke(true);
            p.set_stroke_cap(Cap::Butt);
        };
        draw_arcs(canvas, set_stroke);
    }
);

// Port of: gm/circulararcs.cpp#L109-L116 (chrome/m156)
// `DEF_ARC_GM(stroke_square)`: the manifest id is the macro argument, the GM is named `circular_arcs_stroke_square`.
crate::def_simple_gm_bg_name!(
    stroke_square,
    canvas,
    W,
    H,
    Color::WHITE,
    "circular_arcs_stroke_square",
    {
        let set_stroke = |p: &mut Paint| {
            p.set_stroke(true);
            p.set_stroke_cap(Cap::Square);
        };
        draw_arcs(canvas, set_stroke);
    }
);

// Port of: gm/circulararcs.cpp#L118-L125 (chrome/m156)
// `DEF_ARC_GM(stroke_round)`: the manifest id is the macro argument, the GM is named `circular_arcs_stroke_round`.
crate::def_simple_gm_bg_name!(
    stroke_round,
    canvas,
    W,
    H,
    Color::WHITE,
    "circular_arcs_stroke_round",
    {
        let set_stroke = |p: &mut Paint| {
            p.set_stroke(true);
            p.set_stroke_cap(Cap::Round);
        };
        draw_arcs(canvas, set_stroke);
    }
);

// Port of: gm/circulararcs.cpp#L127-L212 (chrome/m156)
crate::def_simple_gm!(circular_arcs_weird, canvas, 1000, 400, {
    const S: f32 = 50.0;
    struct Arc {
        oval: Rect,
        start: f32,
        sweep: f32,
    }
    let no_draw_arcs = [
        // no sweep
        Arc {
            oval: Rect::from_wh(S, S),
            start: 0.0,
            sweep: 0.0,
        },
        // empty rect in x
        Arc {
            oval: Rect::from_wh(-S, S),
            start: 0.0,
            sweep: 90.0,
        },
        // empty rect in y
        Arc {
            oval: Rect::from_wh(S, -S),
            start: 0.0,
            sweep: 90.0,
        },
        // empty rect in x and y
        Arc {
            oval: Rect::from_wh(0.0, 0.0),
            start: 0.0,
            sweep: 90.0,
        },
    ];
    let arcs = [
        // large start
        Arc {
            oval: Rect::from_wh(S, S),
            start: 810.0,
            sweep: 90.0,
        },
        // large negative start
        Arc {
            oval: Rect::from_wh(S, S),
            start: -810.0,
            sweep: 90.0,
        },
        // exactly 360 sweep
        Arc {
            oval: Rect::from_wh(S, S),
            start: 0.0,
            sweep: 360.0,
        },
        // exactly -360 sweep
        Arc {
            oval: Rect::from_wh(S, S),
            start: 0.0,
            sweep: -360.0,
        },
        // exactly 540 sweep
        Arc {
            oval: Rect::from_wh(S, S),
            start: 0.0,
            sweep: 540.0,
        },
        // exactly -540 sweep
        Arc {
            oval: Rect::from_wh(S, S),
            start: 0.0,
            sweep: -540.0,
        },
        // generic large sweep and large start
        Arc {
            oval: Rect::from_wh(S, S),
            start: 1125.0,
            sweep: 990.0,
        },
    ];
    let mut paints: Vec<Paint> = Vec::new();
    // fill
    paints.push(Paint::default());
    // stroke
    let mut paint = Paint::default();
    paint.set_stroke(true);
    paint.set_stroke_width(S / 6.0);
    paints.push(paint);
    // hairline
    let mut paint = Paint::default();
    paint.set_stroke(true);
    paint.set_stroke_width(0.0);
    paints.push(paint);
    // stroke and fill
    let mut paint = Paint::default();
    paint.set_style(Style::StrokeAndFill);
    paint.set_stroke_width(S / 6.0);
    paints.push(paint);
    // dash effect
    let mut paint = Paint::default();
    paint.set_stroke(true);
    paint.set_stroke_width(S / 6.0);
    let dash_intervals = [S / 15.0, 2.0 * S / 15.0];
    paint.set_path_effect(dash_path_effect::new(&dash_intervals, 0.0));
    paints.push(paint);

    const PAD: f32 = 20.0;
    canvas.translate((PAD, PAD));
    // This loop should draw nothing.
    for arc in &no_draw_arcs {
        for paint in &paints {
            let mut paint = paint.clone();
            paint.set_anti_alias(true);
            canvas.draw_arc(arc.oval, arc.start, arc.sweep, false, &paint);
            canvas.draw_arc(arc.oval, arc.start, arc.sweep, true, &paint);
        }
    }

    let mut line_paint = Paint::default();
    line_paint.set_anti_alias(true);
    line_paint.set_color(Color::RED);
    #[allow(clippy::cast_precision_loss)] // size_t to SkScalar
    let mid_x = arcs.len() as f32 * (S + PAD) - PAD / 2.0;
    #[allow(clippy::cast_precision_loss)] // size_t to SkScalar
    let height = paints.len() as f32 * (S + PAD);
    canvas.draw_line((mid_x, -PAD), (mid_x, height), &line_paint);

    for paint in &paints {
        let mut paint = paint.clone();
        paint.set_anti_alias(true);
        canvas.save();
        for arc in &arcs {
            canvas.draw_arc(arc.oval, arc.start, arc.sweep, false, &paint);
            canvas.translate((S + PAD, 0.0));
        }
        for arc in &arcs {
            canvas.draw_arc(arc.oval, arc.start, arc.sweep, true, &paint);
            canvas.translate((S + PAD, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, S + PAD));
    }
});

// Port of: gm/circulararcs.cpp#L214-L236 (chrome/m156)
crate::def_simple_gm!(onebadarc, canvas, 100, 100, {
    let mut path = PathBuilder::new();
    path.move_to((bits_to_float(0x41a00000), bits_to_float(0x41a00000))); // 20, 20
    path.line_to((bits_to_float(0x4208918c), bits_to_float(0x4208918c))); // 34.1421f, 34.1421f
    path.conic_to(
        (bits_to_float(0x41a00000), bits_to_float(0x42412318)), // 20, 48.2843f
        (bits_to_float(0x40bb73a0), bits_to_float(0x4208918c)), // 5.85786f, 34.1421f
        bits_to_float(0x3f3504f3),                              // 0.707107f
    );
    path.quad_to(
        (bits_to_float(0x40bb73a0), bits_to_float(0x4208918c)), // 5.85786f, 34.1421f
        (bits_to_float(0x40bb73a2), bits_to_float(0x4208918c)), // 5.85787f, 34.1421f
    );
    path.line_to((bits_to_float(0x41a00000), bits_to_float(0x41a00000))); // 20, 20
    path.close();
    let mut p0 = Paint::default();
    p0.set_color(Color::RED);
    p0.set_stroke_width(15.0);
    p0.set_stroke(true);
    p0.set_alpha(100);
    canvas.translate((20.0, 0.0));
    canvas.draw_path(&path.detach(), &p0);

    canvas.draw_arc(Rect::new(60.0, 0.0, 100.0, 40.0), 45.0, 90.0, true, &p0);
});

// Two GPU path renderers were using a too-large tolerance when chopping connics to quads.
// This manifested as not-very-round circular arcs at certain radii. All the arcs being drawn
// here should look like circles.
// Port of: gm/circulararcs.cpp#L238-L257 (chrome/m156)
crate::def_simple_gm!(crbug_888453, canvas, 480, 150, {
    let mut fill = Paint::default();
    fill.set_anti_alias(true);
    let mut hairline = fill.clone();
    hairline.set_stroke(true);
    let mut stroke = hairline.clone();
    stroke.set_stroke_width(2.0);
    let mut x: i32 = 4;
    let (y0, y1, y2): (i32, i32, i32) = (25, 75, 125);
    for r in 2..=20 {
        #[allow(clippy::cast_precision_loss)] // int to SkScalar
        let make = |y: i32| {
            Rect::from_xywh(
                (x - r) as f32,
                (y - r) as f32,
                (2 * r) as f32,
                (2 * r) as f32,
            )
        };
        canvas.draw_arc(make(y0), 0.0, 360.0, false, &fill);
        canvas.draw_arc(make(y1), 0.0, 360.0, false, &hairline);
        canvas.draw_arc(make(y2), 0.0, 360.0, false, &stroke);
        x += 2 * r + 4;
    }
});

// Port of: gm/circulararcs.cpp#L259-L337 (chrome/m156)
crate::def_simple_gm!(circular_arc_stroke_matrix, canvas, 820, 1090, {
    const RADIUS: f32 = 40.0;
    const STROKE_WIDTH: f32 = 5.0;
    const START: f32 = 89.0;
    const SWEEP: f32 = 180.0 / SCALAR_PI; // one radian

    let mut matrices: Vec<Matrix> = Vec::new();
    // Note: the C++ passes (kRadius, kRadius, 45.f) to setRotate(degrees, px, py).
    let mut m = Matrix::new_identity();
    m.set_rotate(RADIUS, Some(Point::new(RADIUS, 45.0)));
    matrices.push(m);
    matrices.push(Matrix::i().clone());
    matrices.push(Matrix::new_all(
        -1.0,
        0.0,
        2.0 * RADIUS,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ));
    matrices.push(Matrix::new_all(
        1.0,
        0.0,
        0.0,
        0.0,
        -1.0,
        2.0 * RADIUS,
        0.0,
        0.0,
        1.0,
    ));
    matrices.push(Matrix::new_all(
        1.0,
        0.0,
        0.0,
        0.0,
        -1.0,
        2.0 * RADIUS,
        0.0,
        0.0,
        1.0,
    ));
    matrices.push(Matrix::new_all(
        0.0,
        -1.0,
        2.0 * RADIUS,
        -1.0,
        0.0,
        2.0 * RADIUS,
        0.0,
        0.0,
        1.0,
    ));
    matrices.push(Matrix::new_all(
        0.0,
        -1.0,
        2.0 * RADIUS,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ));
    matrices.push(Matrix::new_all(0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0));
    matrices.push(Matrix::new_all(
        0.0,
        1.0,
        0.0,
        -1.0,
        0.0,
        2.0 * RADIUS,
        0.0,
        0.0,
        1.0,
    ));
    let base_matrix_cnt = matrices.len();

    let mut tiny_cw = Matrix::new_identity();
    tiny_cw.set_rotate(0.001, Some(Point::new(RADIUS, RADIUS)));
    for i in 0..base_matrix_cnt {
        let m = Matrix::concat(&matrices[i], &tiny_cw);
        matrices.push(m);
    }
    let mut tiny_ccw = Matrix::new_identity();
    tiny_ccw.set_rotate(-0.001, Some(Point::new(RADIUS, RADIUS)));
    for i in 0..base_matrix_cnt {
        let m = Matrix::concat(&matrices[i], &tiny_ccw);
        matrices.push(m);
    }
    let mut cw45 = Matrix::new_identity();
    cw45.set_rotate(45.0, Some(Point::new(RADIUS, RADIUS)));
    for i in 0..base_matrix_cnt {
        let m = Matrix::concat(&matrices[i], &cw45);
        matrices.push(m);
    }

    let mut x: usize = 0;
    let mut y: usize = 0;
    const PAD: f32 = 2.0 * STROKE_WIDTH;
    canvas.translate((PAD, PAD));
    let bounds = Rect::from_wh(2.0 * RADIUS, 2.0 * RADIUS);
    for cap in [Cap::Round, Cap::Butt, Cap::Square] {
        for m in &matrices {
            let mut paint = Paint::default();
            paint.set_stroke_cap(cap);
            paint.set_anti_alias(true);
            paint.set_stroke(true);
            paint.set_stroke_width(STROKE_WIDTH);
            canvas.save();
            #[allow(clippy::cast_precision_loss)] // int to SkScalar
            canvas.translate((
                x as f32 * (2.0 * RADIUS + PAD),
                y as f32 * (2.0 * RADIUS + PAD),
            ));
            canvas.concat(m);
            paint.set_color(Color::RED);
            paint.set_alpha(0x80);
            canvas.draw_arc(bounds, START, SWEEP, false, &paint);
            paint.set_color(Color::BLUE);
            paint.set_alpha(0x80);
            canvas.draw_arc(bounds, START, SWEEP - 360.0, false, &paint);
            canvas.restore();
            x += 1;
            if x == base_matrix_cnt {
                x = 0;
                y += 1;
            }
        }
    }
});

// Port of: gm/circulararcs.cpp#L339-L366 (chrome/m156)
crate::def_simple_gm!(crbug_1472747, canvas, 400, 400, {
    let add_canvas2d_circle_arc_to = |cx: f32, cy: f32, radius: f32, builder: &mut PathBuilder| {
        let oval = Rect::from_ltrb(cx - radius, cy - radius, cx + radius, cy + radius);
        // arcTo(oval, 0, 2pi, anticlockwise) gets split to 0->-180,-180->-360
        builder.arc_to(oval, 0.0, -180.0, false);
        builder.arc_to(oval, -180.0, -180.0, false);
    };

    // This manually stroked circle is large enough to trigger pre-chopping in the
    // tessellation path renderers, but uses a non-default winding mode, which
    // originally was not preserved in the chopped path.
    const RADIUS: f32 = 31000.0;
    let mut stroked_circle = PathBuilder::new();
    add_canvas2d_circle_arc_to(0.0, RADIUS + 10.0, RADIUS, &mut stroked_circle); // inner
    add_canvas2d_circle_arc_to(0.0, RADIUS + 10.0, RADIUS + 5.0, &mut stroked_circle); // outer
    stroked_circle.set_fill_type(PathFillType::EvenOdd);

    let mut fill = Paint::default();
    fill.set_anti_alias(true);
    canvas.draw_path(&stroked_circle.detach(), &fill);
});

// Port of: gm/circulararcs.cpp#L368-L388 (chrome/m156)
crate::def_simple_gm!(bug406747427, canvas, 400, 400, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);
    paint.set_stroke_cap(Cap::Round);
    paint.set_color(Color::from_argb(255, 255, 0, 0));
    paint.set_stroke_width(50.0);
    let mut oval = Rect::from_xywh(100.0, 40.0, 50.0, 50.0);
    canvas.draw_arc(oval, 45.0, 275.0, false, &paint);

    paint.set_color(Color::from_argb(255, 0, 0, 255));
    paint.set_stroke_width(48.0);
    oval = Rect::from_xywh(100.0, 140.0, 50.0, 50.0);
    canvas.draw_arc(oval, 45.0, 275.0, false, &paint);

    paint.set_color(Color::from_argb(255, 0, 255, 0));
    paint.set_stroke_width(80.0);
    oval = Rect::from_xywh(100.0, 280.0, 50.0, 50.0);
    canvas.draw_arc(oval, 45.0, 275.0, false, &paint);
});
