// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/trickycubicstrokes.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::geometry::chop_cubic_at;
use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

const STROKE_WIDTH: f32 = 30.0;
const CELL_SIZE: i32 = 200;
const NUM_COLS: i32 = 5;
const NUM_ROWS: i32 = 5;
const TEST_WIDTH: i32 = NUM_COLS * CELL_SIZE;
const TEST_HEIGHT: i32 = NUM_ROWS * CELL_SIZE;

// Port of: gm/trickycubicstrokes.cpp#L33-L36 (chrome/m156)
#[derive(Clone, Copy, PartialEq, Eq)]
enum CellFillMode {
    Stretch,
    Center,
}

// Port of: gm/trickycubicstrokes.cpp#L38-L45 (chrome/m156)
// Entries with 3 points are quads/conics: pts[3].x holds the conic weight (1 == quad) and
// pts[3].y is unused.
#[derive(Clone, Copy)]
struct TrickyCubic {
    pts: [(f32, f32); 4],
    fill_mode: CellFillMode,
    num_pts: usize,
    scale: f32,
}

// Port of: gm/trickycubicstrokes.cpp#L47-L94 (chrome/m156)
// This is a compilation of cubics that have given strokers grief. Feel free to add more.
// `(NaN)` is the unused fourth point of the quads and conics.
const NAN: f32 = f32::NAN;
#[allow(clippy::excessive_precision)] // literals as written in the C++ source
const TRICKY_CUBICS: [TrickyCubic; 23] = {
    use CellFillMode::{Center as C, Stretch as S};
    [
        TrickyCubic {
            pts: [
                (122.0, 737.0),
                (348.0, 553.0),
                (403.0, 761.0),
                (400.0, 760.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCubic {
            pts: [
                (244.0, 520.0),
                (244.0, 518.0),
                (1141.0, 634.0),
                (394.0, 688.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCubic {
            pts: [
                (550.0, 194.0),
                (138.0, 130.0),
                (1035.0, 246.0),
                (288.0, 300.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCubic {
            pts: [
                (226.0, 733.0),
                (556.0, 779.0),
                (-43.0, 471.0),
                (348.0, 683.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCubic {
            pts: [
                (268.0, 204.0),
                (492.0, 304.0),
                (352.0, 23.0),
                (433.0, 412.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCubic {
            pts: [
                (172.0, 480.0),
                (396.0, 580.0),
                (256.0, 299.0),
                (338.0, 677.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCubic {
            pts: [
                (731.0, 340.0),
                (318.0, 252.0),
                (1026.0, -64.0),
                (367.0, 265.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCubic {
            pts: [
                (475.0, 708.0),
                (62.0, 620.0),
                (770.0, 304.0),
                (220.0, 659.0),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Perfect cusp
        TrickyCubic {
            pts: [(0.0, 0.0), (128.0, 128.0), (128.0, 0.0), (0.0, 128.0)],
            fill_mode: C,
            num_pts: 4,
            scale: 1.0,
        },
        // Near-cusp
        TrickyCubic {
            pts: [(0.0, 0.01), (128.0, 127.999), (128.0, 0.01), (0.0, 127.99)],
            fill_mode: C,
            num_pts: 4,
            scale: 1.0,
        },
        // Near-cusp
        TrickyCubic {
            pts: [
                (0.0, -0.01),
                (128.0, 128.001),
                (128.0, -0.01),
                (0.0, 128.001),
            ],
            fill_mode: C,
            num_pts: 4,
            scale: 1.0,
        },
        // Flat line with 180
        TrickyCubic {
            pts: [(0.0, 0.0), (0.0, -10.0), (0.0, -10.0), (0.0, 10.0)],
            fill_mode: C,
            num_pts: 4,
            scale: 1.098_283,
        },
        // Flat line with 2 180s
        TrickyCubic {
            pts: [(10.0, 0.0), (0.0, 0.0), (20.0, 0.0), (10.0, 0.0)],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Flat diagonal with 180
        TrickyCubic {
            pts: [(39.0, -39.0), (40.0, -40.0), (40.0, -40.0), (0.0, 0.0)],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Near-flat diagonal
        TrickyCubic {
            pts: [(39.0, -39.0), (40.0, -40.0), (37.0, -39.0), (0.0, 0.0)],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Diag w/ an internal 180
        TrickyCubic {
            pts: [(40.0, 40.0), (0.0, 0.0), (200.0, 200.0), (0.0, 0.0)],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Circle
        TrickyCubic {
            pts: [(0.0, 0.0), (1e-2, 0.0), (-1e-2, 0.0), (0.0, 0.0)],
            fill_mode: C,
            num_pts: 4,
            scale: 1.0,
        },
        // Flat line with no turns
        TrickyCubic {
            pts: [
                (400.75, 100.05),
                (400.75, 100.05),
                (100.05, 300.95),
                (100.05, 300.95),
            ],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Flat line with 2 180s
        TrickyCubic {
            pts: [(0.5, 0.0), (0.0, 0.0), (20.0, 0.0), (10.0, 0.0)],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Flat line with a 180
        TrickyCubic {
            pts: [(10.0, 0.0), (0.0, 0.0), (10.0, 0.0), (10.0, 0.0)],
            fill_mode: S,
            num_pts: 4,
            scale: 1.0,
        },
        // Flat QUAD with a cusp
        TrickyCubic {
            pts: [(1.0, 1.0), (2.0, 1.0), (1.0, 1.0), (1.0, NAN)],
            fill_mode: S,
            num_pts: 3,
            scale: 1.0,
        },
        // Flat CONIC with a cusp
        TrickyCubic {
            pts: [(1.0, 1.0), (100.0, 1.0), (25.0, 1.0), (0.3, NAN)],
            fill_mode: S,
            num_pts: 3,
            scale: 1.0,
        },
        // Flat CONIC with a cusp
        TrickyCubic {
            pts: [(1.0, 1.0), (100.0, 1.0), (25.0, 1.0), (1.5, NAN)],
            fill_mode: S,
            num_pts: 3,
            scale: 1.0,
        },
    ]
};

// C++ std::min / std::max: `min(a, b) == (b < a) ? b : a`, `max(a, b) == (a < b) ? b : a`.
// Port of: <algorithm> std::min / std::max as used by gm/trickycubicstrokes.cpp#L100-L103
fn cmin(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

// Port of: <algorithm> std::max as used by gm/trickycubicstrokes.cpp#L100-L103
fn cmax(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

// Port of: gm/trickycubicstrokes.cpp#L97-L112 (chrome/m156)
fn calc_tight_cubic_bounds(p: &[Point], depth: i32) -> Rect {
    if 0 == depth {
        return Rect::from_ltrb(
            cmin(cmin(p[0].x, p[1].x), cmin(p[2].x, p[3].x)),
            cmin(cmin(p[0].y, p[1].y), cmin(p[2].y, p[3].y)),
            cmax(cmax(p[0].x, p[1].x), cmax(p[2].x, p[3].x)),
            cmax(cmax(p[0].y, p[1].y), cmax(p[2].y, p[3].y)),
        );
    }

    let mut chopped = [Point::default(); 7];
    chop_cubic_at(p, &mut chopped, 0.5);
    let mut bounds = calc_tight_cubic_bounds(&chopped[0..4], depth - 1);
    bounds.join(calc_tight_cubic_bounds(&chopped[3..7], depth - 1));
    bounds
}

// Port of: gm/trickycubicstrokes.cpp#L114-L117 (chrome/m156)
fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point::new((b.x - a.x) * t + a.x, (b.y - a.y) * t + a.y)
}

// Port of: gm/trickycubicstrokes.cpp#L124-L184 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int cell coordinates, as in C++
#[allow(clippy::too_many_lines)]
fn draw_test(canvas: &Canvas, cap: Cap, join: Join) {
    let mut rand = Random::default();

    canvas.clear(Color::BLACK);

    let mut stroke_paint = Paint::default();
    stroke_paint.set_anti_alias(true);
    stroke_paint.set_stroke_width(STROKE_WIDTH);
    stroke_paint.set_style(Style::Stroke);
    stroke_paint.set_stroke_cap(cap);
    stroke_paint.set_stroke_join(join);

    for (i, entry) in TRICKY_CUBICS.iter().enumerate() {
        let num_pts = entry.num_pts;
        let fill_mode = entry.fill_mode;
        let scale = entry.scale;

        let mut p = [Point::default(); 4];
        for j in 0..num_pts {
            p[j] = Point::new(entry.pts[j].0 * scale, entry.pts[j].1 * scale);
        }
        let w = entry.pts[3].0;

        let cell_rect = Rect::from_xywh(
            ((i as i32 % NUM_COLS) * CELL_SIZE) as f32,
            ((i as i32 / NUM_COLS) * CELL_SIZE) as f32,
            CELL_SIZE as f32,
            CELL_SIZE as f32,
        );

        let mut stroke_bounds = if num_pts == 4 {
            calc_tight_cubic_bounds(&p, 5)
        } else {
            let as_cubic = [
                p[0],
                lerp(p[0], p[1], 2.0_f32 / 3.0),
                lerp(p[1], p[2], 1.0_f32 / 3.0),
                p[2],
            ];
            calc_tight_cubic_bounds(&as_cubic, 5)
        };
        stroke_bounds.outset((STROKE_WIDTH, STROKE_WIDTH));

        let matrix = if fill_mode == CellFillMode::Stretch {
            Matrix::rect_to_rect_or_identity(stroke_bounds, cell_rect, ScaleToFit::Center)
        } else {
            Matrix::translate((
                cell_rect.left + STROKE_WIDTH + (cell_rect.width() - stroke_bounds.width()) / 2.0,
                cell_rect.top + STROKE_WIDTH + (cell_rect.height() - stroke_bounds.height()) / 2.0,
            ))
        };

        let _acr = AutoCanvasRestore::guard(canvas, true);
        canvas.concat(&matrix);
        stroke_paint.set_stroke_width(STROKE_WIDTH / matrix.max_scale());
        stroke_paint.set_color(Color::new(rand.next_u() | 0xff80_8080));
        let mut builder = PathBuilder::new();
        builder.move_to(p[0]);
        if num_pts == 4 {
            builder.cubic_to(p[1], p[2], p[3]);
        } else if w == 1.0 {
            builder.quad_to(p[1], p[2]);
        } else {
            builder.conic_to(p[1], p[2], w);
        }
        canvas.draw_path(&builder.detach(), &stroke_paint);
    }
}

// Port of: gm/trickycubicstrokes.cpp#L186-L188 (chrome/m156)
crate::def_simple_gm!(trickycubicstrokes, canvas, TEST_WIDTH, TEST_HEIGHT, {
    draw_test(canvas, Cap::Butt, Join::Miter);
});

// Port of: gm/trickycubicstrokes.cpp#L190-L192 (chrome/m156)
crate::def_simple_gm!(
    trickycubicstrokes_roundcaps,
    canvas,
    TEST_WIDTH,
    TEST_HEIGHT,
    {
        draw_test(canvas, Cap::Round, Join::Round);
    }
);

// See b/433057370
// Port of: gm/trickycubicstrokes.cpp#L194-L218 (chrome/m156)
crate::def_simple_gm!(trickycubicstrokes_largeradius, canvas, 128, 256, {
    let mut b = PathBuilder::new();

    // Starts as a line with a single tangent direction, with increasing curvature
    for y in 0..2 {
        #[allow(clippy::cast_precision_loss)] // int loop index, as in C++
        let shift = 210.0 * y as f32;
        #[allow(clippy::cast_precision_loss)] // int loop index, as in C++
        let dy = 5.0 * y as f32;
        b.move_to((159.429, 149.808 + shift)).cubic_to(
            (232.5, 149.808 + dy + shift),
            (232.5, 149.808 + dy + shift),
            (305.572, 149.808 + shift),
        );
    }

    // A large stroke width is required to show the cusp circle artifacts with
    // the tessellating path renderer
    let mut s = Paint::default();
    s.set_stroke(true);
    s.set_stroke_width(200.0);
    s.set_anti_alias(true);
    b.set_fill_type(PathFillType::Winding);
    canvas.scale((0.5, 0.5));
    canvas.translate((-125.0, 0.0));
    canvas.draw_path(&b.detach(), &s);
});
