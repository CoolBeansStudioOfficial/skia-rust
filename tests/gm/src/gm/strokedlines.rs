// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/strokedlines.cpp (chrome/m156)

// The int-to-scalar casts of small constants mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::color_to_565;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::colors;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{SCALAR_PI, scalar, scalar_cos, scalar_sin};
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::dash_path_effect;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/strokedlines.cpp#L11-L16 (chrome/m156), constants
const K_NUM_COLUMNS: i32 = 6;
const K_NUM_ROWS: i32 = 8;
const K_RADIUS: i32 = 40; // radius of the snowflake
const K_PAD: i32 = 5; // padding on both sides of the snowflake
const K_NUM_SPOKES: i32 = 6;
const K_STROKE_WIDTH: scalar = 5.0;

// Port of: gm/strokedlines.cpp#L18-L25 (chrome/m156), draw_line
fn draw_line(canvas: &Canvas, p0: Point, p1: Point, paint: &Paint, use_draw_path: bool) {
    if use_draw_path {
        canvas.draw_path(&Path::line(p0, p1), paint);
    } else {
        canvas.draw_line(p0, p1, paint);
    }
}

// Port of: gm/strokedlines.cpp#L27-L44 (chrome/m156), draw_fins
fn draw_fins(canvas: &Canvas, offset: Point, angle: scalar, paint: &Paint, use_draw_path: bool) {
    // first fin
    let mut sin = scalar_sin(angle + (SCALAR_PI / 4.0));
    let mut cos = scalar_cos(angle + (SCALAR_PI / 4.0));
    sin *= K_RADIUS as scalar / 2.0;
    cos *= K_RADIUS as scalar / 2.0;

    draw_line(
        canvas,
        offset,
        Point::new(offset.x + cos, offset.y + sin),
        paint,
        use_draw_path,
    );

    // second fin
    sin = scalar_sin(angle - (SCALAR_PI / 4.0));
    cos = scalar_cos(angle - (SCALAR_PI / 4.0));
    sin *= K_RADIUS as scalar / 2.0;
    cos *= K_RADIUS as scalar / 2.0;

    draw_line(
        canvas,
        offset,
        Point::new(offset.x + cos, offset.y + sin),
        paint,
        use_draw_path,
    );
}

// draw a snowflake centered at the origin
// Port of: gm/strokedlines.cpp#L47-L72 (chrome/m156), draw_snowflake
fn draw_snowflake(canvas: &Canvas, paint: &Paint, use_draw_path: bool) {
    let r = (K_RADIUS + K_PAD) as scalar;
    canvas.clip_rect(Rect::from_ltrb(-r, -r, r, r), None, None);

    let mut angle: scalar = 0.0;
    for _ in 0..K_NUM_SPOKES / 2 {
        let mut sin = scalar_sin(angle);
        let mut cos = scalar_cos(angle);
        sin *= K_RADIUS as scalar;
        cos *= K_RADIUS as scalar;

        // main spoke
        draw_line(
            canvas,
            Point::new(-cos, -sin),
            Point::new(cos, sin),
            paint,
            use_draw_path,
        );

        // fins on positive side
        let pos_offset = Point::new(0.5 * cos, 0.5 * sin);
        draw_fins(canvas, pos_offset, angle, paint, use_draw_path);

        // fins on negative side
        let neg_offset = Point::new(-0.5 * cos, -0.5 * sin);
        draw_fins(canvas, neg_offset, angle + SCALAR_PI, paint, use_draw_path);

        angle += SCALAR_PI / (K_NUM_SPOKES / 2) as scalar;
    }
}

// Port of: gm/strokedlines.cpp#L74-L91 (chrome/m156), draw_row
fn draw_row(canvas: &Canvas, paint: &Paint, local_matrix: &Matrix, use_draw_path: bool) {
    canvas.translate(((K_RADIUS + K_PAD) as scalar, 0.0));

    for cap in [Cap::Butt, Cap::Round, Cap::Square] {
        for is_aa in [true, false] {
            let mut tmp = paint.clone();
            tmp.set_stroke_width(K_STROKE_WIDTH);
            tmp.set_style(Style::Stroke);
            tmp.set_stroke_cap(cap);
            tmp.set_anti_alias(is_aa);

            let save_count = canvas.save();
            canvas.concat(local_matrix);
            draw_snowflake(canvas, &tmp, use_draw_path);
            canvas.restore_to_count(save_count);

            canvas.translate(((2 * (K_RADIUS + K_PAD)) as scalar, 0.0));
        }
    }
}

// This GM exercises the special case of a stroked lines.
// Various shaders are applied to ensure the coordinate spaces work out right.
// Port of: gm/strokedlines.cpp#L93-L199 (chrome/m156), class StrokedLinesGM
#[derive(Debug)]
pub struct StrokedLinesGm {
    use_draw_path: bool,
    paints: Vec<Paint>,
    matrices: Vec<Matrix>,
}

impl StrokedLinesGm {
    // Port of: gm/strokedlines.cpp#L97-L99 (chrome/m156), StrokedLinesGM(bool)
    #[must_use]
    pub fn new(use_draw_path: bool) -> Self {
        Self {
            use_draw_path,
            paints: Vec::new(),
            matrices: Vec::new(),
        }
    }
}

impl GM for StrokedLinesGm {
    // Port of: gm/strokedlines.cpp (chrome/m156), getName
    fn name(&self) -> String {
        // To preserve history, useDrawPath==true has no suffix.
        let mut name = "strokedlines".to_owned();
        if !self.use_draw_path {
            name.push_str("_drawPoints");
        }
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(
            K_NUM_COLUMNS * (2 * K_RADIUS + 2 * K_PAD),
            K_NUM_ROWS * (2 * K_RADIUS + 2 * K_PAD),
        )
    }

    fn bg_color(&self) -> Color {
        color_to_565(0xFF1A_65D7)
    }

    // Port of: gm/strokedlines.cpp#L110-L172 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // paints
        {
            // basic white
            let mut p = Paint::default();
            p.set_color(Color::WHITE);
            self.paints.push(p);
        }
        {
            // gradient
            let grad_colors = [colors::RED, colors::GREEN];
            let pts = [
                Point::new(-(K_RADIUS + K_PAD) as scalar, -(K_RADIUS + K_PAD) as scalar),
                Point::new((K_RADIUS + K_PAD) as scalar, (K_RADIUS + K_PAD) as scalar),
            ];
            let grad = Gradient::new(
                Colors::new(&grad_colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            );

            let mut p = Paint::default();
            p.set_shader(shaders::linear_gradient((pts[0], pts[1]), &grad, None));

            self.paints.push(p);
        }
        {
            // dashing
            let intervals = [K_STROKE_WIDTH, K_STROKE_WIDTH];
            let mut p = Paint::default();
            p.set_color(Color::WHITE);
            p.set_path_effect(dash_path_effect::new(&intervals, K_STROKE_WIDTH));

            self.paints.push(p);
        }
        {
            // Bitmap shader
            let mut bm = Bitmap::new();
            bm.alloc_n32_pixels((2, 2), None);
            bm.set_addr32(0, 0, 0xFFFF_FFFF);
            bm.set_addr32(1, 1, 0xFFFF_FFFF);
            bm.set_addr32(1, 0, 0x0);
            bm.set_addr32(0, 1, 0x0);

            let mut m = Matrix::new_identity();
            m.set_rotate(12.0, None);
            m.pre_scale((3.0, 3.0), None);

            let mut p = Paint::default();
            p.set_shader(bm.to_shader(
                (TileMode::Repeat, TileMode::Repeat),
                SamplingOptions::default(),
                &m,
            ));
            self.paints.push(p);
        }
        {
            // blur
            let mut p = Paint::default();
            p.set_color(Color::WHITE);
            p.set_mask_filter(MaskFilter::blur(BlurStyle::Outer, 3.0, None));
            self.paints.push(p);
        }

        // matrices
        {
            // rotation
            let mut m = Matrix::new_identity();
            m.set_rotate(12.0, None);

            self.matrices.push(m);
        }
        {
            // skew
            let mut m = Matrix::new_identity();
            m.set_skew((0.3, 0.5), None);

            self.matrices.push(m);
        }
        {
            // perspective
            let mut m = Matrix::new_identity();
            m.set_persp_x(-1.0 / 300.0);
            m.set_persp_y(1.0 / 300.0);

            self.matrices.push(m);
        }

        assert_eq!(K_NUM_ROWS as usize, self.paints.len() + self.matrices.len());
    }

    // Port of: gm/strokedlines.cpp#L174-L192 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((0.0, (K_RADIUS + K_PAD) as scalar));

        for i in 0..self.paints.len() {
            let save_count = canvas.save();
            draw_row(
                canvas,
                &self.paints[i],
                &Matrix::new_identity(),
                self.use_draw_path,
            );
            canvas.restore_to_count(save_count);

            canvas.translate((0.0, (2 * (K_RADIUS + K_PAD)) as scalar));
        }

        for i in 0..self.matrices.len() {
            let save_count = canvas.save();
            draw_row(
                canvas,
                &self.paints[0],
                &self.matrices[i],
                self.use_draw_path,
            );
            canvas.restore_to_count(save_count);

            canvas.translate((0.0, (2 * (K_RADIUS + K_PAD)) as scalar));
        }
    }
}

// Port of: gm/strokedlines.cpp#L195-L196 (chrome/m156), DEF_GM(return new StrokedLinesGM(true);)
crate::def_gm!(
    StrokedLinesGM_true = "StrokedLinesGM(true)",
    StrokedLinesGm::new(true)
);
// Port of: gm/strokedlines.cpp#L196-L196 (chrome/m156), DEF_GM(return new StrokedLinesGM(false);)
crate::def_gm!(
    StrokedLinesGM_false = "StrokedLinesGM(false)",
    StrokedLinesGm::new(false)
);

// Port of: gm/strokedlines.cpp#L201-L201 (chrome/m156), kStrokeWidth (the second, 20)
const K_CAPS_STROKE_WIDTH: scalar = 20.0;

// Port of: gm/strokedlines.cpp#L203-L215 (chrome/m156), draw_path
fn draw_path_caps(canvas: &Canvas, p0: Point, p1: Point, cap: Cap) {
    // Add a gradient *not* aligned with the line's points to show local coords are tracked properly
    let k_rect = Rect::from_ltrb(
        -K_CAPS_STROKE_WIDTH,
        -K_CAPS_STROKE_WIDTH,
        2.0 * K_CAPS_STROKE_WIDTH,
        4.0 * K_CAPS_STROKE_WIDTH,
    );
    let k_pts = [
        Point::new(k_rect.left, k_rect.top),
        Point::new(k_rect.right, k_rect.bottom),
    ];
    let k_colors = [colors::RED, colors::GREEN, colors::BLUE];
    let k_stops: [scalar; 3] = [0.0, 0.75, 1.0];
    let grad = Gradient::new(
        Colors::new(&k_colors, Some(&k_stops), TileMode::Clamp, None),
        Interpolation::default(),
    );
    let shader: Option<Shader> = shaders::linear_gradient((k_pts[0], k_pts[1]), &grad, None);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);

    paint.set_shader(shader);
    paint.set_stroke_width(K_CAPS_STROKE_WIDTH);
    paint.set_stroke_cap(cap);
    canvas.draw_line(p0, p1, &paint);

    // Show outline and control points
    let (fill_path, _) = fill_path_with_paint_to_path(&Path::line(p0, p1), &paint);

    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);
    paint.set_shader(None);
    paint.set_color(Color::RED);
    canvas.draw_path(&fill_path, &paint);

    paint.set_stroke_width(3.0);
    paint.set_stroke_cap(Cap::Square);
    canvas.draw_points(PointMode::Points, fill_path.points(), &paint);
}

// Port of: gm/strokedlines.cpp#L217-L247 (chrome/m156), DEF_SIMPLE_GM(strokedline_caps)
crate::def_simple_gm!(strokedline_caps, canvas, 1400, 740, {
    canvas.translate((
        K_CAPS_STROKE_WIDTH * 3.0 / 2.0,
        K_CAPS_STROKE_WIDTH * 3.0 / 2.0,
    ));

    let k_caps = [Cap::Square, Cap::Butt, Cap::Round];

    let k_lengths: [scalar; 4] = [
        4.0 * K_CAPS_STROKE_WIDTH,
        K_CAPS_STROKE_WIDTH,
        K_CAPS_STROKE_WIDTH / 2.0,
        K_CAPS_STROKE_WIDTH / 4.0,
    ];

    for cap in k_caps {
        let acr = AutoCanvasRestore::guard(canvas, true);

        let draw_line_at = |x0: scalar, y0: scalar, x1: scalar, y1: scalar| {
            draw_path_caps(canvas, Point::new(x0, y0), Point::new(x1, y1), cap);
            canvas.translate((x0.max(x1) + 2.0 * K_CAPS_STROKE_WIDTH, 0.0));
        };

        for l in k_lengths {
            draw_line_at(0.0, 0.0, l, l);
            draw_line_at(l, l, 0.0, 0.0);
            draw_line_at(l / 2.0, 0.0, l / 2.0, l);
            draw_line_at(0.0, l / 2.0, l, l / 2.0);
        }

        draw_line_at(
            K_CAPS_STROKE_WIDTH / 2.0,
            K_CAPS_STROKE_WIDTH / 2.0,
            K_CAPS_STROKE_WIDTH / 2.0,
            K_CAPS_STROKE_WIDTH / 2.0,
        );

        drop(acr);
        canvas.translate((0.0, k_lengths[0] + 2.0 * K_CAPS_STROKE_WIDTH));
    }
});
