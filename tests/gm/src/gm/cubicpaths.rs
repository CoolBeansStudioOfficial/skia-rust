// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/cubicpaths.cpp (chrome/m156)

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
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color_priv::ColorConverter;
use skia_rust_core::font::Font;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

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
        Self::draw_one(
            canvas,
            &self.flipped,
            &Rect::from_ltrb(0.0, 0.0, 150.0, 80.0),
        );
        canvas.translate((170.0, 0.0));
        Self::draw_one(
            canvas,
            &self.flipped,
            &Rect::from_ltrb(0.0, 0.0, 100.0, 80.0),
        );
        canvas.translate((170.0, 0.0));
        Self::draw_one(
            canvas,
            &self.flipped,
            &Rect::from_ltrb(0.0, 0.0, 150.0, 30.0),
        );
        canvas.translate((170.0, 0.0));
        Self::draw_one(
            canvas,
            &self.flipped,
            &Rect::from_ltrb(0.0, 0.0, 150.0, 10.0),
        );
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
    builder
        .move_to((500.988, 155.200))
        .line_to((526.109, 155.200));

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

// Port of: gm/cubicpaths.cpp#L132-L147 (chrome/m156), CubicPathGM::drawPath (the same in
// CubicClosePathGM, #L253-L268)
#[allow(clippy::too_many_arguments)] // mirrors the C++ drawPath signature
fn draw_cubic_path(
    path: &mut Path,
    canvas: &Canvas,
    color: Color,
    clip: &Rect,
    cap: Cap,
    join: Join,
    style: Style,
    fill: PathFillType,
    stroke_width: scalar,
) {
    path.set_fill_type(fill);
    let mut paint = Paint::default();
    paint.set_stroke_cap(cap);
    paint.set_stroke_width(stroke_width);
    paint.set_stroke_join(join);
    paint.set_color(color);
    paint.set_style(style);
    canvas.save();
    canvas.clip_rect(clip, ClipOp::Intersect, false);
    canvas.draw_path(path, &paint);
    canvas.restore();
}

// Port of: gm/cubicpaths.cpp#L149-L245 (chrome/m156), the onDraw grid shared by CubicPathGM and
// CubicClosePathGM. `title` is the GM's title, `path` its path and name.
fn draw_cubic_grid(canvas: &Canvas, path: &mut Path, title: &str) {
    let fills: [(PathFillType, &str); 4] = [
        (PathFillType::Winding, "Winding"),
        (PathFillType::EvenOdd, "Even / Odd"),
        (PathFillType::InverseWinding, "Inverse Winding"),
        (PathFillType::InverseEvenOdd, "Inverse Even / Odd"),
    ];
    let styles: [(Style, &str); 3] = [
        (Style::Fill, "Fill"),
        (Style::Stroke, "Stroke"),
        (Style::StrokeAndFill, "Stroke And Fill"),
    ];
    let caps: [(Cap, Join, &str); 3] = [
        (Cap::Butt, Join::Bevel, "Butt"),
        (Cap::Round, Join::Round, "Round"),
        (Cap::Square, Join::Bevel, "Square"),
    ];

    let mut title_paint = Paint::default();
    title_paint.set_color(Color::BLACK);
    title_paint.set_anti_alias(true);
    let mut font = Font::from_size(default_portable_typeface(), 15.0);
    canvas.draw_str(title, (20.0, 20.0), &font, &title_paint);

    // `SkRandom rand;` is constructed in C++ but never used.
    let rect = Rect::from_xywh(0.0, 0.0, 100.0, 30.0);
    canvas.save();
    canvas.translate((10.0, 30.0));
    canvas.save();
    for (cap_index, (cap, join, cap_name)) in caps.iter().enumerate() {
        if 0 < cap_index {
            canvas.translate(((rect.width() + 40.0) * styles.len() as scalar, 0.0));
        }
        canvas.save();
        for (fill_index, (fill, fill_name)) in fills.iter().enumerate() {
            if 0 < fill_index {
                canvas.translate((0.0, rect.height() + 40.0));
            }
            canvas.save();
            for (style_index, (style, style_name)) in styles.iter().enumerate() {
                if 0 < style_index {
                    canvas.translate((rect.width() + 40.0, 0.0));
                }
                // Unlike linepaths, this GM uses the raw color, not color_to_565.
                let color = Color::from(0xff00_7000_u32);
                draw_cubic_path(path, canvas, color, &rect, *cap, *join, *style, *fill, 10.0);
                let mut rect_paint = Paint::default();
                rect_paint.set_color(Color::BLACK);
                rect_paint.set_style(Style::Stroke);
                rect_paint.set_stroke_width(-1.0);
                rect_paint.set_anti_alias(true);
                canvas.draw_rect(rect, &rect_paint);

                let mut label_paint = Paint::default();
                label_paint.set_color(color);
                font.set_size(10.0);
                canvas.draw_str(style_name, (0.0, rect.height() + 12.0), &font, &label_paint);
                canvas.draw_str(fill_name, (0.0, rect.height() + 24.0), &font, &label_paint);
                canvas.draw_str(cap_name, (0.0, rect.height() + 36.0), &font, &label_paint);
            }
            canvas.restore();
        }
        canvas.restore();
    }
    canvas.restore();
    canvas.restore();
}

// Port of: gm/cubicpaths.cpp#L127-L246 (chrome/m156), CubicPathGM
struct CubicPathGm;

impl GM for CubicPathGm {
    fn name(&self) -> String {
        "cubicpath".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1240, 390)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut path = PathBuilder::new()
            .move_to((25.0, 10.0))
            .cubic_to((40.0, 20.0), (60.0, 20.0), (75.0, 10.0))
            .detach();
        draw_cubic_grid(
            canvas,
            &mut path,
            "Cubic Drawn Into Rectangle Clips With Indicated Style, Fill and Linecaps, with stroke width 10",
        );
    }
}

// Port of: gm/cubicpaths.cpp#L248-L268 (chrome/m156), CubicClosePathGM
struct CubicClosePathGm;

impl GM for CubicClosePathGm {
    fn name(&self) -> String {
        "cubicclosepath".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1240, 390)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut path = PathBuilder::new()
            .move_to((25.0, 10.0))
            .cubic_to((40.0, 20.0), (60.0, 20.0), (75.0, 10.0))
            .close()
            .detach();
        draw_cubic_grid(
            canvas,
            &mut path,
            "Cubic Closed Drawn Into Rectangle Clips With Indicated Style, Fill and Linecaps, with stroke width 10",
        );
    }
}

// Port of: gm/cubicpaths.cpp#L542 (chrome/m156)
crate::def_gm!(CubicPathGM, CubicPathGm);
// Port of: gm/cubicpaths.cpp#L544 (chrome/m156)
crate::def_gm!(CubicClosePathGM, CubicClosePathGm);

// Port of: gm/cubicpaths.cpp#L371-L382 (chrome/m156), CubicPathShaderGM::drawPath
#[allow(clippy::too_many_arguments)] // mirrors the C++ drawPath signature
fn draw_cubic_shader_path(
    path: &mut Path,
    canvas: &Canvas,
    clip: &Rect,
    cap: Cap,
    join: Join,
    style: Style,
    fill: PathFillType,
    stroke_width: f32,
) {
    let s: f32 = 50.0;
    let pts = [Point::new(0.0, 0.0), Point::new(s, s)];
    let pos: [f32; 3] = [0.0, 1.0 / 2.0, 1.0];
    let conv = ColorConverter::new(&[
        Color::from(0x80F0_0080),
        Color::from(0xF0F0_8000),
        Color::from(0x8000_80F0),
    ]);

    path.set_fill_type(fill);

    let mut paint = Paint::default();
    paint.set_stroke_cap(cap);
    paint.set_stroke_width(stroke_width);
    paint.set_stroke_join(join);
    paint.set_shader(gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(conv.colors4f(), Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    paint.set_style(style);
    canvas.save();
    canvas.clip_rect(*clip, None, None);
    canvas.draw_path(path, &paint);
    canvas.restore();
}

// Port of: gm/cubicpaths.cpp#L371-L540 (chrome/m156), CubicPathShaderGM
struct CubicPathShaderGm;

impl GM for CubicPathShaderGm {
    fn name(&self) -> String {
        "cubicpath_shader".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1240, 390)
    }

    // Port of: gm/cubicpaths.cpp#L400-L538 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let fills: [(PathFillType, &str); 4] = [
            (PathFillType::Winding, "Winding"),
            (PathFillType::EvenOdd, "Even / Odd"),
            (PathFillType::InverseWinding, "Inverse Winding"),
            (PathFillType::InverseEvenOdd, "Inverse Even / Odd"),
        ];
        let styles: [(Style, &str); 3] = [
            (Style::Fill, "Fill"),
            (Style::Stroke, "Stroke"),
            (Style::StrokeAndFill, "Stroke And Fill"),
        ];
        let caps: [(Cap, Join, &str); 3] = [
            (Cap::Butt, Join::Bevel, "Butt"),
            (Cap::Round, Join::Round, "Round"),
            (Cap::Square, Join::Bevel, "Square"),
        ];

        let mut path = PathBuilder::new()
            .move_to((25.0, 10.0))
            .cubic_to((40.0, 20.0), (60.0, 20.0), (75.0, 10.0))
            .detach();

        let mut title_paint = Paint::default();
        title_paint.set_color(Color::BLACK);
        title_paint.set_anti_alias(true);
        let mut font = Font::from_size(default_portable_typeface(), 15.0);
        let title = "Cubic Drawn Into Rectangle Clips With Indicated Style, Fill and Linecaps, \
                     with stroke width 10";
        canvas.draw_str(title, (20.0, 20.0), &font, &title_paint);

        let rect = Rect::new(0.0, 0.0, 100.0, 30.0);
        canvas.save();
        canvas.translate((10.0, 30.0));
        canvas.save();
        for (cap_index, (cap, join, _)) in caps.iter().enumerate() {
            if 0 < cap_index {
                canvas.translate(((rect.width() + 40.0) * styles.len() as f32, 0.0));
            }
            canvas.save();
            for (fill_index, (fill, _)) in fills.iter().enumerate() {
                if 0 < fill_index {
                    canvas.translate((0.0, rect.height() + 40.0));
                }
                canvas.save();
                for (style_index, (style, _)) in styles.iter().enumerate() {
                    if 0 < style_index {
                        canvas.translate((rect.width() + 40.0, 0.0));
                    }
                    let color = Color::from(0xff00_7000);
                    draw_cubic_shader_path(
                        &mut path, canvas, &rect, *cap, *join, *style, *fill, 10.0,
                    );
                    let mut rect_paint = Paint::default();
                    rect_paint.set_color(Color::BLACK);
                    rect_paint.set_style(Style::Stroke);
                    rect_paint.set_stroke_width(-1.0);
                    rect_paint.set_anti_alias(true);
                    canvas.draw_rect(rect, &rect_paint);
                    let mut label_paint = Paint::default();
                    label_paint.set_color(color);
                    font.set_size(10.0);
                    canvas.draw_str(
                        styles[style_index].1,
                        (0.0, rect.height() + 12.0),
                        &font,
                        &label_paint,
                    );
                    canvas.draw_str(
                        fills[fill_index].1,
                        (0.0, rect.height() + 24.0),
                        &font,
                        &label_paint,
                    );
                    canvas.draw_str(
                        caps[cap_index].2,
                        (0.0, rect.height() + 36.0),
                        &font,
                        &label_paint,
                    );
                }
                canvas.restore();
            }
            canvas.restore();
        }
        canvas.restore();
        canvas.restore();
    }
}

// Port of: gm/cubicpaths.cpp#L371 (chrome/m156), DEF_GM( return new CubicPathShaderGM; )
crate::def_gm!(CubicPathShaderGM, CubicPathShaderGm);
