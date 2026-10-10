// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/offsetimagefilter.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::{create_checkerboard_image, int_to_scalar};
use skia_rust_core::color::Color;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_effects::image_filters::{image_sampled, offset};
use skia_rust_tools::font_tool_utils::create_string_bitmap;

const WIDTH: i32 = 600;
const HEIGHT: i32 = 100;
const MARGIN: i32 = 12;

// Port of: gm/offsetimagefilter.cpp#L30-L58 (chrome/m156), OffsetImageFilterGM::DrawClippedImage
fn draw_clipped_image(canvas: &Canvas, image: &Image, paint: &Paint, scale: f32, crop_rect: IRect) {
    let clip_rect = Rect::from_wh(int_to_scalar(image.width()), int_to_scalar(image.height()));
    canvas.save();
    canvas.clip_rect(clip_rect, None, None);
    canvas.scale((scale, scale));
    canvas.draw_image_with_sampling_options(
        image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(paint),
    );
    canvas.restore();

    // Draw a boundary rect around the intersection of the clip rect and crop rect.
    let mut scale_matrix = Matrix::new_identity();
    scale_matrix.set_scale((scale, scale), None);
    let (crop_rect_float, _) = scale_matrix.map_rect(Rect::from(crop_rect));
    let mut clip = clip_rect;
    if clip.intersect(crop_rect_float) {
        let mut stroke_paint = Paint::default();
        stroke_paint.set_style(Style::Stroke);
        stroke_paint.set_stroke_width(2.0);
        stroke_paint.set_color(Color::RED);
        canvas.draw_rect(clip, &stroke_paint);
    }
}

// Port of: gm/offsetimagefilter.cpp#L15-L58 (chrome/m156), OffsetImageFilterGM
struct OffsetImageFilterGm {
    bitmap: Option<Image>,
    checkerboard: Option<Image>,
}

impl GM for OffsetImageFilterGm {
    fn name(&self) -> String {
        "offsetimagefilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/offsetimagefilter.cpp#L24-L28 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.bitmap =
            create_string_bitmap(80, 80, Color::new(0xD000_D000), 15, 65, 96, "e").as_image();
        self.checkerboard = Some(create_checkerboard_image(
            80,
            80,
            Color::from(0xFFA0_A0A0),
            Color::from(0xFF40_4040),
            8,
        ));
    }

    // Port of: gm/offsetimagefilter.cpp#L30-L58 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::BLACK);
        let (Some(bitmap), Some(checkerboard)) = (self.bitmap.clone(), self.checkerboard.clone())
        else {
            return;
        };
        let mut paint = Paint::default();
        for i in 0..4 {
            let image = if i & 0x01 != 0 {
                checkerboard.clone()
            } else {
                bitmap.clone()
            };
            let crop_rect = IRect::from_xywh(
                i * 12,
                i * 8,
                image.width() - i * 8,
                image.height() - i * 12,
            );
            let tile_input = image_sampled(
                Some(image.clone()),
                SamplingOptions::from(FilterMode::Nearest),
            );
            let dx = int_to_scalar(i * 5);
            let dy = int_to_scalar(i * 10);
            paint.set_image_filter(offset((dx, dy), tile_input, Some(Rect::from(crop_rect))));
            draw_clipped_image(canvas, &image, &paint, 1.0, crop_rect);
            canvas.translate((int_to_scalar(image.width() + MARGIN), 0.0));
        }
        let crop_rect = IRect::from_xywh(0, 0, 100, 100);
        paint.set_image_filter(offset((-5.0, -10.0), None, Some(Rect::from(crop_rect))));
        draw_clipped_image(canvas, &bitmap, &paint, 2.0, crop_rect);
    }
}

// Port of: gm/offsetimagefilter.cpp#L98 (chrome/m156), DEF_GM( return new OffsetImageFilterGM; )
crate::def_gm!(
    OffsetImageFilterGM,
    OffsetImageFilterGm {
        bitmap: None,
        checkerboard: None
    }
);

// Port of: gm/offsetimagefilter.cpp#L110-L150 (chrome/m156), SimpleOffsetImageFilterGM::doDraw
fn do_draw(
    canvas: &Canvas,
    r: Rect,
    imgf: Option<skia_rust_core::image_filter::ImageFilter>,
    crop_r: Option<IRect>,
    clip_r: Option<Rect>,
) {
    let mut p = Paint::default();
    if let Some(clip_r) = clip_r {
        p.set_color(Color::from(0xFF00_FF00));
        p.set_style(Style::Stroke);
        let mut inset = clip_r;
        inset.inset((0.5, 0.5));
        canvas.draw_rect(inset, &p);
        p.set_style(Style::Fill);
    }
    // Visualize the crop rect for debugging
    if let (Some(_), Some(crop_r)) = (imgf.as_ref(), crop_r) {
        p.set_color(Color::from(0x66FF_00FF));
        p.set_style(Style::Stroke);
        let mut cr = Rect::from(crop_r);
        cr.inset((0.5, 0.5));
        canvas.draw_rect(cr, &p);
        p.set_style(Style::Fill);
    }
    p.set_color(Color::from(0x6600_00FF));
    canvas.draw_rect(r, &p);
    if let Some(clip_r) = clip_r {
        canvas.save();
        canvas.clip_rect(clip_r, None, None);
    }
    if imgf.is_some() {
        p.set_image_filter(imgf);
    }
    p.set_color(Color::from(0x66FF_0000));
    canvas.draw_rect(r, &p);
    if clip_r.is_some() {
        canvas.restore();
    }
}

// Port of: gm/offsetimagefilter.cpp#L152-L202 (chrome/m156), SimpleOffsetImageFilterGM
struct SimpleOffsetImageFilterGm;

impl GM for SimpleOffsetImageFilterGm {
    fn name(&self) -> String {
        "simple-offsetimagefilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 200)
    }

    // Port of: gm/offsetimagefilter.cpp#L163-L199 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let cr0 = IRect::from_wh(40, 40);
        let cr1 = IRect::from_wh(20, 20);
        let cr2 = IRect::from_xywh(40, 0, 40, 40);
        let r = Rect::from(cr0);
        let r2 = Rect::from(cr2);
        canvas.translate((40.0, 40.0));
        canvas.save();
        do_draw(canvas, r, None, None, None);
        canvas.translate((100.0, 0.0));
        do_draw(canvas, r, offset((20.0, 20.0), None, None), None, None);
        canvas.translate((100.0, 0.0));
        do_draw(
            canvas,
            r,
            offset((20.0, 20.0), None, Some(Rect::from(cr0))),
            Some(cr0),
            None,
        );
        canvas.translate((100.0, 0.0));
        do_draw(canvas, r, offset((20.0, 20.0), None, None), None, Some(r));
        canvas.translate((100.0, 0.0));
        do_draw(
            canvas,
            r,
            offset((20.0, 20.0), None, Some(Rect::from(cr1))),
            Some(cr1),
            None,
        );
        let clip_r = Rect::from_xywh(40.0, 40.0, 40.0, 40.0);
        canvas.translate((100.0, 0.0));
        do_draw(
            canvas,
            r,
            offset((20.0, 20.0), None, None),
            None,
            Some(clip_r),
        );
        canvas.restore();

        // 2nd row
        canvas.translate((0.0, 80.0));
        // combos of clip and crop rects that align with src and dst
        // crop==clip==src
        do_draw(
            canvas,
            r,
            offset((40.0, 0.0), None, Some(Rect::from(cr0))),
            Some(cr0),
            Some(r),
        );
        // crop==src, clip==dst
        canvas.translate((100.0, 0.0));
        do_draw(
            canvas,
            r,
            offset((40.0, 0.0), None, Some(Rect::from(cr0))),
            Some(cr0),
            Some(r2),
        );
        // crop==dst, clip==src
        canvas.translate((100.0, 0.0));
        do_draw(
            canvas,
            r,
            offset((40.0, 0.0), None, Some(Rect::from(cr2))),
            Some(cr2),
            Some(r),
        );
        // crop==clip==dst
        canvas.translate((100.0, 0.0));
        do_draw(
            canvas,
            r,
            offset((40.0, 0.0), None, Some(Rect::from(cr2))),
            Some(cr2),
            Some(r2),
        );
    }
}

// Port of: gm/offsetimagefilter.cpp#L205 (chrome/m156), DEF_GM( return new SimpleOffsetImageFilterGM; )
crate::def_gm!(SimpleOffsetImageFilterGM, SimpleOffsetImageFilterGm);
