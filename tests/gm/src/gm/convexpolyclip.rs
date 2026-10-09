// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/convexpolyclip.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements
)]

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color4f;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{SCALAR_PI, scalar_ceil_to_int, scalar_cos, scalar_sin};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
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

// `SkColorConverter`: `SkColor4f::FromColor` of each color.
// Port of: src/core/SkColor.cpp#L177-L182 (chrome/m156)
fn color_converter(src: &[Color]) -> Vec<Color4f> {
    src.iter().map(|&c| Color4f::from_color(c)).collect()
}

// Port of: gm/convexpolyclip.cpp#L10-L62 (chrome/m156)
fn make_img(w: i32, h: i32) -> Option<Image> {
    let mut surf = surfaces::raster(
        &ImageInfo::new_n32((w, h), AlphaType::Opaque, None),
        None,
        None,
    )?;
    let w_scalar = w as f32;
    let h_scalar = h as f32;
    let pt = Point::new(w_scalar / 2.0, h_scalar / 2.0);
    let radius = 3.0 * w_scalar.max(h_scalar);
    let colors = [
        Color::DARK_GRAY,
        color_to_565(0xFF222255),
        color_to_565(0xFF331133),
        color_to_565(0xFF884422),
        color_to_565(0xFF000022),
        Color::WHITE,
        color_to_565(0xFFAABBCC),
    ];
    let colors4f = color_converter(&colors);
    let pos: [f32; 7] = [
        0.0,
        1.0 / 6.0,
        2.0 * 1.0 / 6.0,
        3.0 * 1.0 / 6.0,
        4.0 * 1.0 / 6.0,
        5.0 * 1.0 / 6.0,
        1.0,
    ];
    let mut paint = Paint::default();
    let mut rect = Rect::from_wh(w_scalar, h_scalar);
    let mut mat = Matrix::new_identity();
    {
        let canvas = surf.canvas();
        for _ in 0..4 {
            paint.set_shader(shaders::radial_gradient(
                (pt, radius),
                &Gradient::new(
                    Colors::new(&colors4f, Some(&pos), TileMode::Repeat, None),
                    Interpolation::default(),
                ),
                Some(&mat),
            ));
            canvas.draw_rect(rect, &paint);
            rect.inset((w_scalar / 8.0, h_scalar / 8.0));
            mat.pre_translate((6.0 * w_scalar, 6.0 * h_scalar));
            mat.post_scale((1.0 / 3.0, 1.0 / 3.0), None);
        }
        let font = Font::from_size(default_portable_typeface(), w_scalar / 2.2);
        paint.set_shader(None);
        paint.set_color(Color::LIGHT_GRAY);
        let txt = b"Skia";
        let tex_pos = Point::new(w_scalar / 17.0, h_scalar / 2.0 + font.size() / 2.5);
        canvas.draw_simple_text(txt, TextEncoding::UTF8, tex_pos, &font, &paint);
        paint.set_color(Color::BLACK);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(1.0);
        canvas.draw_simple_text(txt, TextEncoding::UTF8, tex_pos, &font, &paint);
    }
    surf.image_snapshot()
}

// Port of: gm/convexpolyclip.cpp#L81-L87 (chrome/m156)
#[derive(Default)]
enum ClipType {
    #[default]
    None,
    Path,
    Rect,
}

// Port of: gm/convexpolyclip.cpp#L111-L152 (chrome/m156)
struct Clip {
    kind: ClipType,
    path_builder: PathBuilder,
    rect: Rect,
}

impl Clip {
    // Port of: gm/convexpolyclip.cpp#L113-L120 (chrome/m156)
    fn new() -> Self {
        Self {
            kind: ClipType::None,
            path_builder: PathBuilder::new(),
            rect: Rect::default(),
        }
    }

    // Port of: gm/convexpolyclip.cpp#L121-L134 (chrome/m156)
    fn set_on_canvas(&self, canvas: &Canvas, op: ClipOp, aa: bool) {
        match self.kind {
            ClipType::Path => {
                canvas.clip_path(&self.path_builder.snapshot(), op, aa);
            }
            ClipType::Rect => {
                canvas.clip_rect(self.rect, op, aa);
            }
            ClipType::None => {}
        }
    }

    // Port of: gm/convexpolyclip.cpp#L135-L149 (chrome/m156)
    fn as_closed_path(&self) -> Path {
        match self.kind {
            ClipType::Path => PathBuilder::new_path(&self.path_builder.snapshot())
                .close()
                .detach(),
            ClipType::Rect => Path::rect(self.rect, None),
            ClipType::None => Path::default(),
        }
    }

    // Port of: gm/convexpolyclip.cpp#L150-L154 (chrome/m156)
    fn set_path(&mut self, path: &Path) {
        self.kind = ClipType::Path;
        self.path_builder = PathBuilder::new_path(path);
    }

    // Port of: gm/convexpolyclip.cpp#L155-L159 (chrome/m156)
    fn set_rect(&mut self, rect: Rect) {
        self.kind = ClipType::Rect;
        self.rect = rect;
        self.path_builder.reset();
    }

    // Port of: gm/convexpolyclip.cpp#L161-L172 (chrome/m156)
    fn get_bounds(&self) -> Rect {
        match self.kind {
            ClipType::Path => self.path_builder.compute_bounds(),
            ClipType::Rect => self.rect,
            ClipType::None => Rect::default(),
        }
    }
}

// Port of: gm/convexpolyclip.cpp#L64-L220 (chrome/m156)
struct ConvexPolyClipGm {
    clips: Vec<Clip>,
    img: Option<Image>,
}

impl ConvexPolyClipGm {
    fn new() -> Self {
        Self {
            clips: Vec::new(),
            img: None,
        }
    }

    fn img(&self) -> &Image {
        self.img.as_ref().expect("the image is made before drawing")
    }
}

impl GM for ConvexPolyClipGm {
    fn name(&self) -> String {
        "convex_poly_clip".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(435 * 2, 540)
    }

    fn bg_color(&self) -> Color {
        Color::WHITE
    }

    // Port of: gm/convexpolyclip.cpp#L76-L107 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let mut c = Clip::new();
        c.set_path(&Path::polygon(
            &[
                Point::new(5.0, 5.0),
                Point::new(100.0, 20.0),
                Point::new(15.0, 100.0),
            ],
            false,
            None,
            None,
        ));
        self.clips.push(c);

        let mut hexagon = PathBuilder::new();
        let k_radius: f32 = 45.0;
        let center = Point::new(k_radius, k_radius);
        for i in 0..6 {
            let angle = 2.0 * SCALAR_PI * (i as f32) / 6.0;
            let mut point = Point::new(scalar_cos(angle), scalar_sin(angle));
            point.scale(k_radius);
            point = center + point;
            if i == 0 {
                hexagon.move_to(point);
            } else {
                hexagon.line_to(point);
            }
        }
        let mut c = Clip::new();
        c.set_path(&hexagon.snapshot());
        self.clips.push(c);

        let mut scale_m = Matrix::new_identity();
        scale_m.set_scale((1.1, 0.4), Point::new(k_radius, k_radius));
        let mut c = Clip::new();
        c.set_path(&hexagon.detach().make_transform(&scale_m));
        self.clips.push(c);

        let mut c = Clip::new();
        c.set_rect(Rect::from_xywh(8.3, 11.6, 78.2, 72.6));
        self.clips.push(c);

        let rect = Rect::from_ltrb(10.0, 12.0, 80.0, 86.0);
        let mut rot_m = Matrix::new_identity();
        rot_m.set_rotate(23.0, Point::new(rect.center_x(), rect.center_y()));
        let mut c = Clip::new();
        c.set_path(&Path::rect(rect, None).make_transform(&rot_m));
        self.clips.push(c);

        self.img = make_img(100, 100);
    }

    // Port of: gm/convexpolyclip.cpp#L108-L210 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut y: f32 = 0.0;
        let k_margin: f32 = 10.0;
        let mut bg_paint = Paint::default();
        bg_paint.set_alpha(0x15);
        let size = canvas.base_layer_size();
        let img = self.img();
        canvas.draw_image_rect_with_sampling_options(
            img,
            None,
            Rect::from_iwh(size.width, size.height),
            SamplingOptions::default(),
            &bg_paint,
        );
        let txt = b"Clip Me!";
        let font = Font::from_size(default_portable_typeface(), 23.0);
        let text_w = font.measure_text(txt, TextEncoding::UTF8, None).0;
        let mut txt_paint = Paint::default();
        txt_paint.set_color(Color::DARK_GRAY);
        let mut start_x: f32 = 0.0;
        let test_layers = 1;
        let img_w = img.width() as f32;
        let img_h = img.height() as f32;
        for do_layer in 0..=test_layers {
            for clip in &self.clips {
                let mut x = start_x;
                for aa in 0..2 {
                    if do_layer != 0 {
                        let mut bounds = clip.get_bounds();
                        bounds.outset((2.0, 2.0));
                        bounds.offset((x, y));
                        canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
                    } else {
                        canvas.save();
                    }
                    canvas.translate((x, y));
                    clip.set_on_canvas(canvas, ClipOp::Intersect, aa != 0);
                    canvas.draw_image(img, (0.0, 0.0), None);
                    canvas.restore();
                    x += img_w + k_margin;
                }
                for aa in 0..2 {
                    let mut clip_outline_paint = Paint::default();
                    clip_outline_paint.set_anti_alias(true);
                    clip_outline_paint.set_color(Color::new(0x50505050));
                    clip_outline_paint.set_style(Style::Stroke);
                    clip_outline_paint.set_stroke_width(0.0);
                    if do_layer != 0 {
                        let mut bounds = clip.get_bounds();
                        bounds.outset((2.0, 2.0));
                        bounds.offset((x, y));
                        canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
                    } else {
                        canvas.save();
                    }
                    canvas.translate((x, y));
                    let closed_clip_path = clip.as_closed_path();
                    canvas.draw_path(&closed_clip_path, &clip_outline_paint);
                    clip.set_on_canvas(canvas, ClipOp::Intersect, aa != 0);
                    canvas.scale((1.0, 1.8));
                    canvas.draw_simple_text(
                        txt,
                        TextEncoding::UTF8,
                        (0.0, 1.5 * font.size()),
                        &font,
                        &txt_paint,
                    );
                    canvas.restore();
                    x += text_w + 2.0 * k_margin;
                }
                y += img_h + k_margin;
            }
            y = 0.0;
            start_x += (2 * img.width() + scalar_ceil_to_int(2.0 * text_w)) as f32 + 6.0 * k_margin;
        }
    }
}

crate::def_gm!(ConvexPolyClip, ConvexPolyClipGm::new());
