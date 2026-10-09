// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dstreadshuffle.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::Font;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// `ToolUtils::color_to_565`.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
fn color_to_565(color: u32) -> Color {
    use skia_rust_core::color::pre_multiply_color;
    use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
    let pm_color = pre_multiply_color(Color::new(color));
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

// Port of: gm/dstreadshuffle.cpp#L12-L19 (chrome/m156)
#[derive(Clone, Copy)]
enum ShapeType {
    Circle,
    RoundRect,
    Rect,
    ConvexPath,
    ConcavePath,
    Text,
}

// Port of: gm/dstreadshuffle.cpp#L11-L100 (chrome/m156)
// `kBackground` = `SK_ColorLTGRAY`.
const K_BACKGROUND: Color = Color::new(0xFFCCCCCC);

// Port of: gm/dstreadshuffle.cpp#L9-L110 (chrome/m156)
struct DstReadShuffleGm {
    concave_path: Path,
    convex_path: Path,
}

impl DstReadShuffleGm {
    fn new() -> Self {
        Self {
            concave_path: Path::new(),
            convex_path: Path::new(),
        }
    }

    // Port of: gm/dstreadshuffle.cpp#L25-L66 (chrome/m156)
    fn draw_shape(&mut self, canvas: &Canvas, paint: &Paint, shape_type: ShapeType) {
        let k_rect = Rect::from_xywh(0.0, 0.0, 75.0, 85.0);
        match shape_type {
            ShapeType::Circle => {
                canvas.draw_circle(
                    (k_rect.center_x(), k_rect.center_y()),
                    k_rect.width() / 2.0,
                    paint,
                );
            }
            ShapeType::RoundRect => {
                canvas.draw_round_rect(k_rect, 15.0, 15.0, paint);
            }
            ShapeType::Rect => {
                canvas.draw_rect(k_rect, paint);
            }
            ShapeType::ConvexPath => {
                if self.convex_path.is_empty() {
                    let points = k_rect.to_quad(None);
                    self.convex_path = PathBuilder::new()
                        .move_to(points[0])
                        .quad_to(points[1], points[2])
                        .quad_to(points[3], points[0])
                        .detach();
                }
                canvas.draw_path(&self.convex_path, paint);
            }
            ShapeType::ConcavePath => {
                if self.concave_path.is_empty() {
                    let mut b = PathBuilder::new();
                    let mut points = [Point::new(50.0, 0.0); 5];
                    let mut rot = Matrix::new_identity();
                    rot.set_rotate(360.0 / 5.0, Point::new(50.0, 70.0));
                    for i in 1..5 {
                        points[i] = rot.map_point(points[i - 1]);
                    }
                    b.move_to(points[0]);
                    for i in 0..5 {
                        b.line_to(points[(2 * i) % 5]);
                    }
                    self.concave_path = b.set_fill_type(PathFillType::EvenOdd).detach();
                }
                canvas.draw_path(&self.concave_path, paint);
            }
            ShapeType::Text => {
                let mut font = Font::from_size(default_portable_typeface(), 100.0);
                font.set_embolden(true);
                draw_string(canvas, "N", 0.0, 100.0, &font, paint, Align::Left);
            }
        }
    }

    // Port of: gm/dstreadshuffle.cpp#L67-L70 (chrome/m156)
    fn get_color(random: &mut Random) -> Color {
        let color = color_to_565(random.next_u() | 0xFF000000);
        color.with_a(0x80)
    }

    // Port of: gm/dstreadshuffle.cpp#L71-L94 (chrome/m156)
    fn draw_hairlines(canvas: &Canvas) {
        if canvas.image_info().alpha_type() == AlphaType::Opaque {
            canvas.clear(K_BACKGROUND);
        } else {
            canvas.clear(Color::TRANSPARENT);
        }
        let mut hair_paint = Paint::default();
        hair_paint.set_style(Style::Stroke);
        hair_paint.set_stroke_width(0.0);
        hair_paint.set_anti_alias(true);
        const K_NUM_HAIRLINES: i32 = 12;
        let mut pts = [Point::new(3.0, 7.0), Point::new(29.0, 7.0)];
        let mut color_random = Random::default();
        let mut rot = Matrix::new_identity();
        rot.set_rotate(360.0 / K_NUM_HAIRLINES as f32, Point::new(15.5, 12.0));
        rot.post_translate((3.0, 0.0));
        for _ in 0..12 {
            hair_paint.set_color(Self::get_color(&mut color_random));
            canvas.draw_line(pts[0], pts[1], &hair_paint);
            rot.map_points_inplace(&mut pts);
        }
    }
}

impl GM for DstReadShuffleGm {
    fn name(&self) -> String {
        "dstreadshuffle".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(530, 680)
    }

    fn bg_color(&self) -> Color {
        K_BACKGROUND
    }

    // Port of: gm/dstreadshuffle.cpp#L95-L145 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let shape_types = [
            ShapeType::Circle,
            ShapeType::RoundRect,
            ShapeType::Rect,
            ShapeType::ConvexPath,
            ShapeType::ConcavePath,
            ShapeType::Text,
        ];
        let mut y: f32 = 5.0;
        for shape_type in shape_types {
            let mut color_random = Random::default();
            let mut x: f32 = 5.0;
            for r in 0..=15 {
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_color(Self::get_color(&mut color_random));
                p.set_blend_mode(if r % 3 == 0 {
                    BlendMode::ColorBurn
                } else {
                    BlendMode::SrcOver
                });
                canvas.save();
                canvas.translate((x, y));
                self.draw_shape(canvas, &p, shape_type);
                canvas.restore();
                x += 15.0;
            }
            y += 110.0;
        }

        let canvas_info = canvas.image_info();
        let mut info = if canvas_info.color_type() == ColorType::Unknown {
            ImageInfo::new_n32_premul((35, 35), None)
        } else {
            ImageInfo::new(
                (35, 35),
                canvas_info.color_type(),
                canvas_info.alpha_type(),
                canvas_info.color_space(),
            )
        };
        let mut surf = if let Some(surf) = canvas.new_surface(&info, None) {
            surf
        } else {
            if (info.color_type() == ColorType::RGBA8888
                || info.color_type() == ColorType::BGRA8888)
                && info.color_type() != ColorType::N32
            {
                info = ImageInfo::new(
                    (35, 35),
                    ColorType::N32,
                    canvas_info.alpha_type(),
                    canvas_info.color_space(),
                );
            }
            surfaces::raster(&info, None, None).expect("a surface")
        };
        canvas.scale((5.0, 5.0));
        canvas.translate((67.0, 10.0));
        Self::draw_hairlines(surf.canvas());
        let snapshot = surf.image_snapshot().expect("a snapshot");
        canvas.draw_image(&snapshot, (0.0, 0.0), None);
    }
}

crate::def_gm!(DstReadShuffle, DstReadShuffleGm::new());
