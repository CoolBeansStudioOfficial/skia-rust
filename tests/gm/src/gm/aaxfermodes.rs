// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/aaxfermodes.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::draw_checkerboard;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/aaxfermodes.cpp#L14-L22 (chrome/m156), the layout constants
const K_SHAPE_SIZE: i32 = 22;
const K_SHAPE_SPACING: i32 = 36;
const K_SHAPE_TYPE_SPACING: i32 = 4 * K_SHAPE_SPACING / 3;
const K_PAINT_SPACING: i32 = 4 * K_SHAPE_TYPE_SPACING;
const K_LABEL_SPACING: i32 = 3 * K_SHAPE_SIZE;
const K_MARGIN: i32 = K_SHAPE_SPACING / 2;
const K_XFERMODE_TYPE_SPACING: i32 = K_LABEL_SPACING + 2 * K_PAINT_SPACING + K_SHAPE_TYPE_SPACING;
const K_TITLE_SPACING: i32 = 3 * K_SHAPE_SPACING / 4;
const K_SUBTITLE_SPACING: i32 = 5 * K_SHAPE_SPACING / 8;
// Port of: gm/aaxfermodes.cpp#L23 (chrome/m156), kBGColor
const K_BG_COLOR: u32 = 0xc8d2_b887;
// Port of: gm/aaxfermodes.cpp#L24-L27 (chrome/m156), kShapeColors
const K_SHAPE_COLORS: [u32; 2] = [
    0x82ff_0080, // input color unknown
    0xff00_ffff, // input color opaque
];

// Port of: gm/aaxfermodes.cpp#L28-L33 (chrome/m156), enum Shape
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    Square,
    Diamond,
    Oval,
    Concave,
}

const K_SHAPES: [Shape; 4] = [Shape::Square, Shape::Diamond, Shape::Oval, Shape::Concave];

// Port of: gm/aaxfermodes.cpp#L35-L40 (chrome/m156), enum DrawingPass
#[derive(Clone, Copy, PartialEq, Eq)]
enum DrawingPass {
    Checkerboard,
    Background,
    Shape,
}

// Port of: gm/aaxfermodes.cpp#L41-L100 (chrome/m156), class AAXfermodesGM
struct AaxfermodesGm {
    label_font: Font,
    oval: Path,
    concave: Path,
}

impl AaxfermodesGm {
    fn new() -> Self {
        // Port of: gm/aaxfermodes.cpp#L53-L70 (chrome/m156), onOnceBeforeDraw
        let mut label_font = Font::from_size(default_portable_typeface(), 13.0);
        label_font.set_subpixel(true);
        let radius: f32 = -1.4 * K_SHAPE_SIZE as f32 / 2.0;
        let pts = [
            (-radius, 0.0),
            (0.0, -1.33 * radius),
            (radius, 0.0),
            (0.0, 1.33 * radius),
        ];
        let mut oval = PathBuilder::new();
        oval.move_to(pts[0])
            .quad_to(pts[1], pts[2])
            .quad_to(pts[3], pts[0]);
        let mut concave = PathBuilder::new();
        concave
            .move_to((-radius, 0.0))
            .quad_to((0.0, 0.0), (0.0, -radius))
            .quad_to((0.0, 0.0), (radius, 0.0))
            .quad_to((0.0, 0.0), (0.0, radius))
            .quad_to((0.0, 0.0), (-radius, 0.0))
            .close();
        Self {
            label_font,
            oval: oval.detach(),
            concave: concave.detach(),
        }
    }

    // Port of: gm/aaxfermodes.cpp#L102-L154 (chrome/m156), draw_pass
    fn draw_pass(&self, canvas: &Canvas, drawing_pass: DrawingPass) {
        let clip_size = (K_SHAPE_SIZE * 11 / 16) as f32;
        let clip_rect = Rect::from_ltrb(-clip_size, -clip_size, clip_size, clip_size);

        canvas.save();
        if drawing_pass == DrawingPass::Checkerboard {
            canvas.translate((K_MARGIN as f32, K_MARGIN as f32));
        }
        canvas.translate((0.0, K_TITLE_SPACING as f32));
        for xfermode_set in 0..2_usize {
            let first_mode = (BlendMode::LAST_COEFF_MODE as usize + 1) * xfermode_set;
            canvas.save();
            if drawing_pass == DrawingPass::Shape {
                let x = (K_LABEL_SPACING as f32)
                    + (K_SHAPE_TYPE_SPACING as f32) * 1.5
                    + (K_SHAPE_SPACING / 2) as f32;
                let y = (K_SUBTITLE_SPACING / 2) as f32 + self.label_font.size() / 3.0;
                text_utils::draw_string(
                    canvas,
                    "Src Unknown",
                    x,
                    y,
                    &self.label_font,
                    &Paint::default(),
                    Align::Center,
                );
                let x2 = (K_LABEL_SPACING as f32)
                    + (K_SHAPE_TYPE_SPACING as f32) * 1.5
                    + (K_SHAPE_SPACING / 2) as f32
                    + K_PAINT_SPACING as f32;
                text_utils::draw_string(
                    canvas,
                    "Src Opaque",
                    x2,
                    y,
                    &self.label_font,
                    &Paint::default(),
                    Align::Center,
                );
            }
            canvas.translate((0.0, (K_SUBTITLE_SPACING + K_SHAPE_SPACING / 2) as f32));
            for m in 0..=(BlendMode::LAST_COEFF_MODE as usize) {
                if first_mode + m > BlendMode::LAST_MODE as usize {
                    break;
                }
                let mode = blend_mode_from_index(first_mode + m);
                canvas.save();
                if drawing_pass == DrawingPass::Shape {
                    self.draw_mode_name(canvas, mode);
                }
                canvas.translate(((K_LABEL_SPACING + K_SHAPE_SPACING / 2) as f32, 0.0));
                for &color in &K_SHAPE_COLORS {
                    let mut paint = Paint::default();
                    setup_shape_paint(canvas, Color::from(color), mode, &mut paint);

                    canvas.save();
                    for &shape in &K_SHAPES {
                        if drawing_pass != DrawingPass::Shape {
                            canvas.save();
                            canvas.clip_rect(clip_rect, None, None);
                            if drawing_pass == DrawingPass::Checkerboard {
                                draw_checkerboard(
                                    canvas,
                                    Color::from(0xffff_ffff),
                                    Color::from(0xffc6_c3c6),
                                    10,
                                );
                            } else {
                                canvas.draw_color(Color::from(K_BG_COLOR), BlendMode::Src);
                            }
                            canvas.restore();
                        } else {
                            self.draw_shape(canvas, shape, &paint, mode);
                        }
                        canvas.translate((K_SHAPE_TYPE_SPACING as f32, 0.0));
                    }
                    canvas.restore();
                    canvas.translate((K_PAINT_SPACING as f32, 0.0));
                }
                canvas.restore();
                canvas.translate((0.0, K_SHAPE_SPACING as f32));
            }
            canvas.restore();
            canvas.translate((K_XFERMODE_TYPE_SPACING as f32, 0.0));
        }
        canvas.restore();
    }

    // Port of: gm/aaxfermodes.cpp#L156-L163 (chrome/m156), onDraw
    fn draw_all(&self, canvas: &Canvas) {
        self.draw_pass(canvas, DrawingPass::Checkerboard);
        canvas.save_layer(&SaveLayerRec::default());
        canvas.translate((K_MARGIN as f32, K_MARGIN as f32));
        self.draw_pass(canvas, DrawingPass::Background);

        let mut title_font = self.label_font.clone();
        title_font.set_size(9.0 * title_font.size() / 8.0);
        title_font.set_embolden(true);

        text_utils::draw_string(
            canvas,
            "Porter Duff",
            (K_LABEL_SPACING + 4 * K_SHAPE_TYPE_SPACING) as f32,
            (K_TITLE_SPACING / 2) as f32 + title_font.size() / 3.0,
            &title_font,
            &Paint::default(),
            Align::Center,
        );
        text_utils::draw_string(
            canvas,
            "Advanced",
            (K_XFERMODE_TYPE_SPACING + K_LABEL_SPACING + 4 * K_SHAPE_TYPE_SPACING) as f32,
            (K_TITLE_SPACING / 2) as f32 + title_font.size() / 3.0,
            &title_font,
            &Paint::default(),
            Align::Center,
        );

        self.draw_pass(canvas, DrawingPass::Shape);
        canvas.restore();
    }

    // Port of: gm/aaxfermodes.cpp#L165-L170 (chrome/m156), drawModeName
    fn draw_mode_name(&self, canvas: &Canvas, mode: BlendMode) {
        let mode_name = mode.name();
        text_utils::draw_string(
            canvas,
            mode_name,
            (K_LABEL_SPACING - K_SHAPE_SIZE / 4) as f32,
            self.label_font.size() / 4.0,
            &self.label_font,
            &Paint::default(),
            Align::Right,
        );
    }

    // Port of: gm/aaxfermodes.cpp#L219-L251 (chrome/m156), drawShape
    fn draw_shape(&self, canvas: &Canvas, shape: Shape, paint: &Paint, mode: BlendMode) {
        let mut shape_paint = paint.clone();
        shape_paint.set_anti_alias(shape != Shape::Square);
        shape_paint.set_blend_mode(mode);
        let half = (K_SHAPE_SIZE / 2) as f32;
        match shape {
            Shape::Square => {
                canvas.draw_rect(Rect::from_ltrb(-half, -half, half, half), &shape_paint);
            }
            Shape::Diamond => {
                canvas.save();
                canvas.rotate(45.0, None);
                canvas.draw_rect(Rect::from_ltrb(-half, -half, half, half), &shape_paint);
                canvas.restore();
            }
            Shape::Oval => {
                canvas.save();
                let degrees = ((511 * mode as i32 + 257) % 360) as f32;
                canvas.rotate(degrees, None);
                canvas.draw_path(&self.oval, &shape_paint);
                canvas.restore();
            }
            Shape::Concave => {
                canvas.draw_path(&self.concave, &shape_paint);
            }
        }
    }
}

// The `SkBlendMode` value with index `i` (`static_cast<SkBlendMode>(i)`), for `i <= kLastMode`.
// Port of: include/core/SkBlendMode.h (chrome/m156), SkBlendMode
fn blend_mode_from_index(i: usize) -> BlendMode {
    BlendMode::from_i32(i as i32).expect("a blend mode index no greater than kLastMode")
}

// Port of: gm/aaxfermodes.cpp#L172-L217 (chrome/m156), setupShapePaint
fn setup_shape_paint(canvas: &Canvas, color: Color, mode: BlendMode, paint: &mut Paint) {
    paint.set_color(color);
    if mode == BlendMode::Plus {
        let bg = Color::from(K_BG_COLOR);
        // Check for overflow, otherwise we might get confusing AA artifacts.
        let max_sum = (i32::from(bg.a()) + i32::from(color.a()))
            .max(i32::from(bg.r()) + i32::from(color.r()))
            .max(i32::from(bg.g()) + i32::from(color.g()))
            .max(i32::from(bg.b()) + i32::from(color.b()));
        if max_sum > 255 {
            let mut dim_paint = Paint::default();
            dim_paint.set_anti_alias(false);
            dim_paint.set_blend_mode(BlendMode::DstIn);
            if paint.alpha() != 255 {
                // Dim the src and dst colors.
                let dim = (255 * 255 / max_sum) as u8;
                dim_paint.set_argb(dim, 0, 0, 0);
                let alpha = (255 * i32::from(paint.alpha()) / max_sum) as u8;
                paint.set_alpha(alpha);
            } else {
                // Just clear the dst, we need to preserve the paint's opacity.
                dim_paint.set_argb(0, 0, 0, 0);
            }
            let half = K_SHAPE_SPACING / 2;
            canvas.draw_rect(
                Rect::from_ltrb(
                    (-half) as f32,
                    (-half) as f32,
                    (half + 3 * K_SHAPE_TYPE_SPACING) as f32,
                    half as f32,
                ),
                &dim_paint,
            );
        }
    }
}

impl GM for AaxfermodesGm {
    // Port of: gm/aaxfermodes.cpp#L84-L87 (chrome/m156), getName
    fn name(&self) -> String {
        "aaxfermodes".to_string()
    }

    // Port of: gm/aaxfermodes.cpp#L84-L94 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        let width = 2 * K_MARGIN + 2 * K_XFERMODE_TYPE_SPACING
            - (K_XFERMODE_TYPE_SPACING - (K_LABEL_SPACING + 2 * K_PAINT_SPACING));
        let height = 2 * K_MARGIN
            + K_TITLE_SPACING
            + K_SUBTITLE_SPACING
            + (1 + BlendMode::LAST_COEFF_MODE as i32) * K_SHAPE_SPACING;
        ISize::new(width, height)
    }

    // Port of: gm/aaxfermodes.cpp#L156-L163 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        self.draw_all(canvas);
    }
}

// Port of: gm/aaxfermodes.cpp#L277 (chrome/m156)
crate::def_gm!(AAXfermodesGM, AaxfermodesGm::new());
