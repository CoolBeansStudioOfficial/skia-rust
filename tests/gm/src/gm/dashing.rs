// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dashing.cpp (chrome/m156)

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
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::colors;
use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::paint::{Cap, Paint};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    SCALAR_HALF, SCALAR_PI, SCALAR_ROOT_2_OVER_2, scalar_abs, scalar_cos, scalar_sin,
};
use skia_rust_effects::dash_path_effect;

// `ToolUtils::color_to_565`.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
fn color_to_565(color: u32) -> Color {
    use skia_rust_core::color::pre_multiply_color;
    use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
    let pm_color = pre_multiply_color(Color::new(color));
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

// Port of: gm/dashing.cpp#L28-L41 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ function with its default arguments
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
fn drawline(
    canvas: &Canvas,
    on: i32,
    off: i32,
    paint: &Paint,
    final_x: f32,
    final_y: f32,
    phase: f32,
    start_x: f32,
    start_y: f32,
) {
    let mut p = paint.clone();

    let intervals = [on as f32, off as f32];

    p.set_path_effect(dash_path_effect::new(&intervals, phase));
    canvas.draw_line((start_x, start_y), (final_x, final_y), &p);
}

/// `drawline(canvas, on, off, paint)` with all default arguments.
fn drawline_default(canvas: &Canvas, on: i32, off: i32, paint: &Paint) {
    drawline(canvas, on, off, paint, 600.0, 0.0, 0.0, 0.0, 0.0);
}

// earlier bug stopped us from drawing very long single-segment dashes, because
// SkPathMeasure was skipping very small delta-T values (nearlyzero). This is
// now fixes, so this giant dash should appear.
// Port of: gm/dashing.cpp#L45-L49 (chrome/m156)
fn show_giant_dash(canvas: &Canvas) {
    let paint = Paint::default();

    drawline(canvas, 1, 1, &paint, 20.0 * 1000.0, 0.0, 0.0, 0.0, 0.0);
}

// Port of: gm/dashing.cpp#L51-L59 (chrome/m156)
fn show_zero_len_dash(canvas: &Canvas) {
    let mut paint = Paint::default();

    drawline(canvas, 2, 2, &paint, 0.0, 0.0, 0.0, 0.0, 0.0);
    paint.set_stroke(true);
    paint.set_stroke_width(2.0);
    canvas.translate((0.0, 20.0));
    drawline(canvas, 4, 4, &paint, 0.0, 0.0, 0.0, 0.0, 0.0);
}

// Port of: gm/dashing.cpp#L61-L102 (chrome/m156)
struct DashingGm;

impl GM for DashingGm {
    fn name(&self) -> String {
        "dashing".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 340)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        struct Intervals {
            on_interval: i32,
            off_interval: i32,
        }

        let mut paint = Paint::default();
        paint.set_stroke(true);

        canvas.translate((20.0, 20.0));
        canvas.translate((0.0, SCALAR_HALF));
        for width in 0..=2 {
            for data in [
                Intervals {
                    on_interval: 1,
                    off_interval: 1,
                },
                Intervals {
                    on_interval: 4,
                    off_interval: 1,
                },
            ] {
                for aa in [false, true] {
                    let w = width * width * width;
                    paint.set_anti_alias(aa);
                    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
                    paint.set_stroke_width(w as f32);

                    let scale = if w != 0 { w } else { 1 };

                    drawline_default(
                        canvas,
                        data.on_interval * scale,
                        data.off_interval * scale,
                        &paint,
                    );
                    canvas.translate((0.0, 20.0));
                }
            }
        }

        show_giant_dash(canvas);
        canvas.translate((0.0, 20.0));
        show_zero_len_dash(canvas);
        canvas.translate((0.0, 20.0));
        // Draw 0 on, 0 off dashed line
        paint.set_stroke_width(8.0);
        drawline_default(canvas, 0, 0, &paint);
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: gm/dashing.cpp#L106-L118 (chrome/m156)
fn make_unit_star(n: i32) -> Path {
    let mut rad: f32 = -SCALAR_PI / 2.0;
    #[allow(clippy::cast_precision_loss)] // (n >> 1) * SK_ScalarPI * 2 / n
    let drad: f32 = (n >> 1) as f32 * SCALAR_PI * 2.0 / n as f32;

    let mut b = PathBuilder::new();
    b.move_to((0.0, -1.0));
    for _ in 1..n {
        rad += drad;
        b.line_to((scalar_cos(rad), scalar_sin(rad)));
    }
    b.close().detach()
}

// Port of: gm/dashing.cpp#L120-L124 (chrome/m156)
fn make_path_line(bounds: &Rect) -> Path {
    PathBuilder::new()
        .move_to((bounds.left, bounds.top))
        .line_to((bounds.right, bounds.bottom))
        .detach()
}

// Port of: gm/dashing.cpp#L126-L128 (chrome/m156)
fn make_path_rect(bounds: &Rect) -> Path {
    Path::rect(bounds, None)
}

// Port of: gm/dashing.cpp#L130-L132 (chrome/m156)
fn make_path_oval(bounds: &Rect) -> Path {
    Path::oval(bounds, None)
}

// Port of: gm/dashing.cpp#L134-L139 (chrome/m156)
fn make_path_star(bounds: &Rect) -> Path {
    let path = make_unit_star(5);
    let matrix = Matrix::rect_to_rect_or_identity(path.bounds(), bounds, ScaleToFit::Center);
    path.make_transform(&matrix)
}

// Port of: gm/dashing.cpp#L141-L190 (chrome/m156)
struct Dashing2Gm;

impl GM for Dashing2Gm {
    fn name(&self) -> String {
        "dashing2".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar, x * dx
    fn on_draw(&mut self, canvas: &Canvas) {
        const G_INTERVALS: [i32; 12] = [
            3, // 3 dashes: each count [0] followed by intervals [1..count]
            2, 10, 10, //
            4, 20, 5, 5, 5, //
            2, 2, 2,
        ];

        let g_proc: [fn(&Rect) -> Path; 4] = [
            make_path_line,
            make_path_rect,
            make_path_oval,
            make_path_star,
        ];

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_stroke(true);
        paint.set_stroke_width(6.0);

        let mut bounds = Rect::from_wh(120.0, 120.0);
        bounds.offset((20.0, 20.0));
        let dx = bounds.width() * 4.0 / 3.0;
        let dy = bounds.height() * 4.0 / 3.0;

        let mut intervals = 1;
        for y in 0..G_INTERVALS[0] {
            let mut vals = [0.0f32; 12]; // more than enough
            let count = G_INTERVALS[intervals] as usize;
            intervals += 1;
            for val in vals.iter_mut().take(count) {
                *val = G_INTERVALS[intervals] as f32;
                intervals += 1;
            }
            let phase = vals[0] / 2.0;
            paint.set_path_effect(dash_path_effect::new(&vals[..count], phase));

            for (x, proc_) in g_proc.iter().enumerate() {
                let mut r = bounds;
                r.offset((x as f32 * dx, y as f32 * dy));
                canvas.draw_path(&proc_(&r), &paint);
            }
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

// Test out the on/off line dashing Chrome if fond of
// Port of: gm/dashing.cpp#L194-L335 (chrome/m156)
struct Dashing3Gm;

impl Dashing3Gm {
    // Draw a 100x100 block of dashed lines. The horizontal ones are BW
    // while the vertical ones are AA.
    // Port of: gm/dashing.cpp#L200-L236 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn draw_dashed_lines(
        canvas: &Canvas,
        line_length: f32,
        phase: f32,
        dash_length: f32,
        stroke_width: i32,
        circles: bool,
    ) {
        let mut p = Paint::default();
        p.set_color(Color::BLACK);
        p.set_stroke(true);
        p.set_stroke_width(stroke_width as f32);

        if circles {
            p.set_stroke_cap(Cap::Round);
        }

        let intervals = [dash_length, dash_length];

        p.set_path_effect(dash_path_effect::new(&intervals, phase));

        let mut pts = [Point::default(); 2];

        let mut y = 0;
        while y < 100 {
            pts[0].set(0.0, y as f32);
            pts[1].set(line_length, y as f32);

            canvas.draw_points(PointMode::Lines, &pts, &p);
            y += 10 * stroke_width;
        }

        p.set_anti_alias(true);

        let mut x = 0;
        while x < 100 {
            pts[0].set(x as f32, 0.0);
            pts[1].set(x as f32, line_length);

            canvas.draw_points(PointMode::Lines, &pts, &p);
            x += 14 * stroke_width;
        }
    }
}

impl GM for Dashing3Gm {
    fn name(&self) -> String {
        "dashing3".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn on_draw(&mut self, canvas: &Canvas) {
        // 1on/1off 1x1 squares with phase of 0 - points fastpath
        canvas.save();
        canvas.translate((2.0, 0.0));
        Self::draw_dashed_lines(canvas, 100.0, 0.0, 1.0, 1, false);
        canvas.restore();

        // 1on/1off 1x1 squares with phase of .5 - rects fastpath (due to partial squares)
        canvas.save();
        canvas.translate((112.0, 0.0));
        Self::draw_dashed_lines(canvas, 100.0, SCALAR_HALF, 1.0, 1, false);
        canvas.restore();

        // 1on/1off 1x1 squares with phase of 1 - points fastpath
        canvas.save();
        canvas.translate((222.0, 0.0));
        Self::draw_dashed_lines(canvas, 100.0, 1.0, 1.0, 1, false);
        canvas.restore();

        // 1on/1off 1x1 squares with phase of 1 and non-integer length - rects fastpath
        canvas.save();
        canvas.translate((332.0, 0.0));
        Self::draw_dashed_lines(canvas, 99.5, SCALAR_HALF, 1.0, 1, false);
        canvas.restore();

        // 255on/255off 1x1 squares with phase of 0 - rects fast path
        canvas.save();
        canvas.translate((446.0, 0.0));
        Self::draw_dashed_lines(canvas, 100.0, 0.0, 255.0, 1, false);
        canvas.restore();

        // 1on/1off 3x3 squares with phase of 0 - points fast path
        canvas.save();
        canvas.translate((2.0, 110.0));
        Self::draw_dashed_lines(canvas, 100.0, 0.0, 3.0, 3, false);
        canvas.restore();

        // 1on/1off 3x3 squares with phase of 1.5 - rects fast path
        canvas.save();
        canvas.translate((112.0, 110.0));
        Self::draw_dashed_lines(canvas, 100.0, 1.5, 3.0, 3, false);
        canvas.restore();

        // 1on/1off 1x1 circles with phase of 1 - no fast path yet
        canvas.save();
        canvas.translate((2.0, 220.0));
        Self::draw_dashed_lines(canvas, 100.0, 1.0, 1.0, 1, true);
        canvas.restore();

        // 1on/1off 3x3 circles with phase of 1 - no fast path yet
        canvas.save();
        canvas.translate((112.0, 220.0));
        Self::draw_dashed_lines(canvas, 100.0, 0.0, 3.0, 3, true);
        canvas.restore();

        // 1on/1off 1x1 squares with rotation - should break fast path
        canvas.save();
        canvas.translate((
            332.0 + SCALAR_ROOT_2_OVER_2 * 100.0,
            110.0 + SCALAR_ROOT_2_OVER_2 * 100.0,
        ));
        canvas.rotate(45.0, None);
        canvas.translate((-50.0, -50.0));

        Self::draw_dashed_lines(canvas, 100.0, 1.0, 1.0, 1, false);
        canvas.restore();

        // 3on/3off 3x1 rects - should use rect fast path regardless of phase
        for phase in 0..=3 {
            canvas.save();
            canvas.translate(((phase * 110 + 2) as f32, 330.0));
            Self::draw_dashed_lines(canvas, 100.0, phase as f32, 3.0, 1, false);
            canvas.restore();
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

// Port of: gm/dashing.cpp#L339-L403 (chrome/m156)
struct Dashing4Gm;

impl GM for Dashing4Gm {
    fn name(&self) -> String {
        "dashing4".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 1100)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn on_draw(&mut self, canvas: &Canvas) {
        struct Intervals {
            on_interval: i32,
            off_interval: i32,
        }

        let mut paint = Paint::default();
        paint.set_stroke(true);

        canvas.translate((20.0, 20.0));
        canvas.translate((SCALAR_HALF, SCALAR_HALF));

        for width in 0..=2 {
            for data in [
                Intervals {
                    on_interval: 1,
                    off_interval: 1,
                },
                Intervals {
                    on_interval: 4,
                    off_interval: 2,
                },
                // test for zero length on interval.
                // zero length intervals should draw
                // a line of squares or circles
                Intervals {
                    on_interval: 0,
                    off_interval: 4,
                },
            ] {
                for aa in [false, true] {
                    for cap in [Cap::Round, Cap::Square] {
                        let w = width * width * width;
                        paint.set_anti_alias(aa);
                        paint.set_stroke_width(w as f32);
                        paint.set_stroke_cap(cap);

                        let scale = if w != 0 { w } else { 1 };

                        drawline_default(
                            canvas,
                            data.on_interval * scale,
                            data.off_interval * scale,
                            &paint,
                        );
                        canvas.translate((0.0, 20.0));
                    }
                }
            }
        }

        for aa in 0..=1 {
            paint.set_anti_alias(aa != 0);
            paint.set_stroke_width(8.0);
            paint.set_stroke_cap(Cap::Square);
            // Single dash element that is cut off at start and end
            drawline(canvas, 32, 16, &paint, 20.0, 0.0, 5.0, 0.0, 0.0);
            canvas.translate((0.0, 20.0));

            // Two dash elements where each one is cut off at beginning and end respectively
            drawline(canvas, 32, 16, &paint, 56.0, 0.0, 5.0, 0.0, 0.0);
            canvas.translate((0.0, 20.0));

            // Many dash elements where first and last are cut off at beginning and end
            // respectively
            drawline(canvas, 32, 16, &paint, 584.0, 0.0, 5.0, 0.0, 0.0);
            canvas.translate((0.0, 20.0));

            // Diagonal dash line where src pnts are not axis aligned (as apposed to being
            // diagonal from a canvas rotation)
            drawline(canvas, 32, 16, &paint, 600.0, 30.0, 0.0, 0.0, 0.0);
            canvas.translate((0.0, 20.0));

            // Case where only the off interval exists on the line. Thus nothing should be drawn
            drawline(canvas, 32, 16, &paint, 8.0, 0.0, 40.0, 0.0, 0.0);
            canvas.translate((0.0, 20.0));
        }

        // Test overlapping circles.
        canvas.translate((5.0, 20.0));
        paint.set_anti_alias(true);
        paint.set_stroke_cap(Cap::Round);
        paint.set_color(Color::new(0x44000000));
        paint.set_stroke_width(40.0);
        drawline_default(canvas, 0, 30, &paint);

        canvas.translate((0.0, 50.0));
        paint.set_stroke_cap(Cap::Square);
        drawline_default(canvas, 0, 30, &paint);

        // Test we draw the cap when the line length is zero.
        canvas.translate((0.0, 50.0));
        paint.set_stroke_cap(Cap::Round);
        paint.set_color(Color::new(0xFF000000));
        paint.set_stroke_width(11.0);
        drawline(canvas, 0, 30, &paint, 0.0, 0.0, 0.0, 0.0, 0.0);
        canvas.translate((100.0, 0.0));
        drawline(canvas, 1, 30, &paint, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
}

//////////////////////////////////////////////////////////////////////////////

// Port of: gm/dashing.cpp#L407-L471 (chrome/m156)
struct Dashing5Gm {
    do_aa: bool,
}

impl Dashing5Gm {
    fn new(do_aa: bool) -> Self {
        Self { do_aa }
    }
}

impl GM for Dashing5Gm {
    fn name(&self) -> String {
        if self.do_aa {
            "dashing5_aa"
        } else {
            "dashing5_bw"
        }
        .to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(400, 200)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn on_draw(&mut self, canvas: &Canvas) {
        const ON: i32 = 4;
        const OFF: i32 = 4;
        const INTERVAL_LENGTH: i32 = ON + OFF;

        let g_colors: [Color; 8] = [
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::CYAN,
            Color::MAGENTA,
            Color::YELLOW,
            Color::GRAY,
            Color::DARK_GRAY,
        ];

        let mut paint = Paint::default();
        paint.set_stroke(true);

        paint.set_anti_alias(self.do_aa);

        let mut rot = Matrix::new_identity();
        rot.set_rotate(90.0, None);
        debug_assert!(rot.rect_stays_rect());

        canvas.concat(&rot);

        let mut phase: i32 = 0;

        let mut x = 0;
        while x < 200 {
            paint.set_stroke_width((phase + 1) as f32);
            paint.set_color(g_colors[phase as usize]);
            let sign: i32 = if x % 20 != 0 { 1 } else { -1 };
            drawline(
                canvas,
                ON,
                OFF,
                &paint,
                x as f32,
                -sign as f32 * 10003.0,
                phase as f32,
                x as f32,
                sign as f32 * 10003.0,
            );
            phase = (phase + 1) % INTERVAL_LENGTH;
            x += 10;
        }

        let mut y = -400;
        while y < 0 {
            paint.set_stroke_width((phase + 1) as f32);
            paint.set_color(g_colors[phase as usize]);
            let sign: i32 = if y % 20 != 0 { 1 } else { -1 };
            drawline(
                canvas,
                ON,
                OFF,
                &paint,
                -sign as f32 * 10003.0,
                y as f32,
                phase as f32,
                sign as f32 * 10003.0,
                y as f32,
            );
            phase = (phase + 1) % INTERVAL_LENGTH;
            y += 10;
        }
    }
}

// Port of: gm/dashing.cpp#L473-L505 (chrome/m156)
crate::def_simple_gm!(
    #[ignore = "see notes/gm_dashing_cpp_longpathdash.md"]
    longpathdash,
    canvas,
    612,
    612,
    {
        let mut lines = PathBuilder::new();
        let mut x: i32 = 32;
        while x < 256 {
            let mut a: f32 = 0.0;
            while a < 3.141592f32 * 2.0 {
                #[allow(clippy::cast_precision_loss)] // int to SkScalar
                let xf = x as f32;
                // skia-rust: libm. `sin` and `cos` here are the double-precision libm functions.
                let pts = [
                    Point::new(
                        256.0 + (f64::from(a).sin() as f32) * xf,
                        256.0 + (f64::from(a).cos() as f32) * xf,
                    ),
                    Point::new(
                        256.0 + ((f64::from(a) + 3.141592 / 3.0).sin() as f32) * (xf + 64.0),
                        256.0 + ((f64::from(a) + 3.141592 / 3.0).cos() as f32) * (xf + 64.0),
                    ),
                ];
                lines.move_to(pts[0]);
                let mut i: f32 = 0.0;
                while i < 1.0 {
                    lines.line_to((
                        pts[0].x * (1.0 - i) + pts[1].x * i,
                        pts[0].y * (1.0 - i) + pts[1].y * i,
                    ));
                    i += 0.05;
                }
                a += 0.03141592;
            }
            x += 16;
        }
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_stroke(true);
        p.set_stroke_width(1.0);
        let intervals = [1.0, 1.0];
        p.set_path_effect(dash_path_effect::new(&intervals, 0.0));

        canvas.translate((50.0, 50.0));
        canvas.draw_path(&lines.detach(), &p);
    }
);

// Port of: gm/dashing.cpp#L507-L517 (chrome/m156)
crate::def_simple_gm!(longlinedash, canvas, 512, 512, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_stroke(true);
    p.set_stroke_width(80.0);

    let intervals = [2.0, 2.0];
    p.set_path_effect(dash_path_effect::new(&intervals, 0.0));
    canvas.draw_rect(Rect::from_xywh(-10000.0, 100.0, 20000.0, 20.0), &p);
});

// Port of: gm/dashing.cpp#L519-L557 (chrome/m156)
crate::def_simple_gm!(dashbigrects, canvas, 256, 256, {
    let mut rand = Random::default();

    const HALF_STROKE_WIDTH: i32 = 8;
    const ON_OFF_INTERVAL: i32 = 2 * HALF_STROKE_WIDTH;

    canvas.clear(colors::BLACK);

    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_stroke(true);
    p.set_stroke_width((2 * HALF_STROKE_WIDTH) as f32);
    p.set_stroke_cap(Cap::Butt);

    let intervals = [ON_OFF_INTERVAL as f32, ON_OFF_INTERVAL as f32];
    p.set_path_effect(dash_path_effect::new(&intervals, 0.0));

    const K: f32 = ON_OFF_INTERVAL as f32;
    const G_WIDTH_HEIGHTS: [f32; 11] = [
        1000000000.0 * K + K / 2.0,
        1000000.0 * K + K / 2.0,
        1000.0 * K + K / 2.0,
        100.0 * K + K / 2.0,
        10.0 * K + K / 2.0,
        9.0 * K + K / 2.0,
        8.0 * K + K / 2.0,
        7.0 * K + K / 2.0,
        6.0 * K + K / 2.0,
        5.0 * K + K / 2.0,
        4.0 * K + K / 2.0,
    ];

    for (i, wh) in G_WIDTH_HEIGHTS.iter().enumerate() {
        p.set_color(color_to_565(rand.next_u() | (0xFF << 24)));

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // size_t to int
        let offset = 2 * i as i32 * HALF_STROKE_WIDTH + HALF_STROKE_WIDTH;
        #[allow(clippy::cast_precision_loss)] // int to SkScalar
        let offset = offset as f32;
        canvas.draw_rect(Rect::from_xywh(offset, offset, *wh, *wh), &p);
    }
});

// Port of: gm/dashing.cpp#L559-L572 (chrome/m156)
crate::def_simple_gm!(longwavyline, canvas, 512, 512, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_stroke(true);
    p.set_stroke_width(2.0);

    let mut wavy = PathBuilder::new();
    wavy.move_to((-10000.0, 100.0));
    let mut i: f32 = -10000.0;
    while i < 10000.0 {
        wavy.quad_to((i + 5.0, 95.0), (i + 10.0, 100.0));
        wavy.quad_to((i + 15.0, 105.0), (i + 20.0, 100.0));
        i += 20.0;
    }
    canvas.draw_path(&wavy.detach(), &p);
});

// Port of: gm/dashing.cpp#L590-L624 (chrome/m156)
crate::def_simple_gm!(dash_line_zero_off_interval, canvas, 160, 330, {
    const INTERVALS: [f32; 4] = [5.0, 0.0, 2.0, 0.0];
    let mut dash_paint = Paint::default();
    dash_paint.set_path_effect(dash_path_effect::new(&INTERVALS, 0.0));
    debug_assert!(dash_paint.path_effect().is_some());
    dash_paint.set_stroke(true);
    dash_paint.set_stroke_width(20.0);
    struct Line {
        a: Point,
        b: Point,
    }
    let lines = [
        Line {
            a: Point::new(0.5, 0.5),
            b: Point::new(30.5, 0.5),
        }, // horizontal
        Line {
            a: Point::new(0.5, 0.5),
            b: Point::new(0.5, 30.5),
        }, // vertical
        Line {
            a: Point::new(0.5, 0.5),
            b: Point::new(0.5, 0.5),
        }, // point
        Line {
            a: Point::new(0.5, 0.5),
            b: Point::new(25.5, 25.5),
        }, // diagonal
    ];
    let pad = 5.0 + dash_paint.stroke_width();
    canvas.translate((pad / 2.0, pad / 2.0));
    canvas.save();
    let mut h: f32 = 0.0;
    for line in &lines {
        h = h.max(scalar_abs(line.a.y - line.b.y));
    }
    for line in &lines {
        let w = scalar_abs(line.a.x - line.b.x);
        for cap in [Cap::Butt, Cap::Square, Cap::Round] {
            dash_paint.set_stroke_cap(cap);
            for aa in [false, true] {
                dash_paint.set_anti_alias(aa);
                canvas.draw_line(line.a, line.b, &dash_paint);
                canvas.translate((0.0, pad + h));
            }
        }
        canvas.restore();
        canvas.translate((pad + w, 0.0));
        canvas.save();
    }
});

// Port of: gm/dashing.cpp#L626-L647 (chrome/m156)
crate::def_simple_gm!(thin_aa_dash_lines, canvas, 330, 110, {
    let mut paint = Paint::default();
    const SCALE: f32 = 100.0;
    const INTERVALS: [f32; 2] = [10.0 / SCALE, 5.0 / SCALE];
    paint.set_path_effect(dash_path_effect::new(&INTERVALS, 0.0));
    paint.set_anti_alias(true);
    paint.set_stroke_width(0.25 / SCALE);
    // substep moves the subpixel offset every iteration.
    const SUBSTEP: f32 = 0.05 / SCALE;
    // We will draw a grid of horiz/vertical lines that pass through each other's off intervals.
    const STEP: f32 = INTERVALS[0] + INTERVALS[1];
    canvas.scale((SCALE, SCALE));
    canvas.translate((INTERVALS[1], INTERVALS[1]));
    for c in [Cap::Butt, Cap::Square, Cap::Round] {
        paint.set_stroke_cap(c);
        let mut x: f32 = -0.5 * INTERVALS[1];
        while x < 105.0 / SCALE {
            canvas.draw_line((x, 0.0), (x, 100.0 / SCALE), &paint);
            canvas.draw_line((0.0, x), (100.0 / SCALE, x), &paint);
            x += STEP + SUBSTEP;
        }
        canvas.translate((110.0 / SCALE, 0.0));
    }
});

// Port of: gm/dashing.cpp#L649-L670 (chrome/m156)
crate::def_simple_gm!(path_effect_empty_result, canvas, 100, 100, {
    let mut p = Paint::default();
    p.set_stroke(true);
    p.set_stroke_width(1.0);

    let mut path = PathBuilder::new();
    let r: f32 = 70.0;
    let l: f32 = 70.0;
    let t: f32 = 70.0;
    let b: f32 = 70.0;
    path.move_to((l, t));
    path.line_to((r, t));
    path.line_to((r, b));
    path.line_to((l, b));
    path.close();

    let dashes = [2.0, 2.0];
    p.set_path_effect(dash_path_effect::new(&dashes, 0.0));

    canvas.draw_path(&path.detach(), &p);
});

//////////////////////////////////////////////////////////////////////////////

// Port of: gm/dashing.cpp#L674-L679 (chrome/m156)
crate::def_gm!(DashingGM, DashingGm);
crate::def_gm!(Dashing2GM, Dashing2Gm);
crate::def_gm!(Dashing3GM, Dashing3Gm);
crate::def_gm!(Dashing4GM, Dashing4Gm);
crate::def_gm!(Dashing5GM_true = "Dashing5GM(true)", Dashing5Gm::new(true));
crate::def_gm!(
    Dashing5GM_false = "Dashing5GM(false)",
    Dashing5Gm::new(false)
);
