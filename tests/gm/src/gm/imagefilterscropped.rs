// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefilterscropped.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss, clippy::too_many_lines)]

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_effects::image_filters::{blur, color_filter, erode, merge, offset};
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/imagefilterscropped.cpp#L30-L38 (chrome/m156), draw_paint
fn draw_paint(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    paint.set_color(Color::BLACK);
    canvas.save();
    canvas.clip_rect(r, None, None);
    canvas.draw_paint(&paint);
    canvas.restore();
}

// Port of: gm/imagefilterscropped.cpp#L40-L47 (chrome/m156), draw_path
fn draw_path(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_color(Color::MAGENTA);
    paint.set_image_filter(imf);
    paint.set_anti_alias(true);
    canvas.draw_circle((r.center_x(), r.center_y()), r.width() * 2.0 / 5.0, &paint);
}

// Port of: gm/imagefilterscropped.cpp#L49-L56 (chrome/m156), draw_text
fn draw_text(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    paint.set_color(Color::GREEN);
    let font = Font::from_size(default_portable_typeface(), r.height() / 2.0);
    text_utils::draw_string(
        canvas,
        "Text",
        r.center_x(),
        r.center_y(),
        &font,
        &paint,
        Align::Center,
    );
}

// Port of: gm/imagefilterscropped.cpp#L58-L68 (chrome/m156), draw_bitmap
fn draw_bitmap(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    let bounds: IRect = RoundOut::<IRect>::round_out(&r);
    let info = ImageInfo::new(
        (bounds.width(), bounds.height()),
        ColorType::N32,
        AlphaType::Premul,
        None,
    );
    let mut surf = surfaces::raster(&info, None, None).expect("a surface");
    draw_path(surf.canvas(), r, None);
    let image = surf.image_snapshot().expect("a snapshot");

    paint.set_image_filter(imf);
    canvas.draw_image_with_sampling_options(
        &image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&paint),
    );
}

type DrawProc = fn(&Canvas, Rect, Option<ImageFilter>);

// Port of: gm/imagefilterscropped.cpp#L78-L80 (chrome/m156), make_checkerboard
fn make_checkerboard() -> Option<Image> {
    let info = ImageInfo::new((80, 80), ColorType::N32, AlphaType::Premul, None);
    let mut surf = surfaces::raster(&info, None, None)?;
    {
        let canvas = surf.canvas();
        let mut dark_paint = Paint::default();
        dark_paint.set_color(Color::from(0xFF40_4040));
        let mut light_paint = Paint::default();
        light_paint.set_color(Color::from(0xFFA0_A0A0));
        for y in (0..80).step_by(16) {
            for x in (0..80).step_by(16) {
                canvas.save();
                canvas.translate((int_to_scalar(x), int_to_scalar(y)));
                canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 8.0, 8.0), &dark_paint);
                canvas.draw_rect(Rect::from_xywh(8.0, 0.0, 8.0, 8.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(0.0, 8.0, 8.0, 8.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(8.0, 8.0, 8.0, 8.0), &dark_paint);
                canvas.restore();
            }
        }
    }
    surf.image_snapshot()
}

// Port of: gm/imagefilterscropped.cpp#L82-L180 (chrome/m156), ImageFiltersCroppedGM
struct ImageFiltersCroppedGm {
    checkerboard: Option<Image>,
}

impl GM for ImageFiltersCroppedGm {
    fn name(&self) -> String {
        "imagefilterscropped".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(400, 960)
    }

    // Port of: gm/imagefilterscropped.cpp#L113-L115 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.checkerboard = make_checkerboard();
    }

    // Port of: gm/imagefilterscropped.cpp#L117-L178 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(checkerboard) = self.checkerboard.clone() else {
            return;
        };
        let draw_proc: [DrawProc; 4] = [draw_bitmap, draw_path, draw_paint, draw_text];

        let cf = color_filters::blend(Color4f::from(Color::BLUE), None, BlendMode::SrcIn);
        let crop_rect = IRect::from_xywh(10, 10, 44, 44);
        let bogus_rect = IRect::from_xywh(-100, -100, 10, 10);
        let crop = Rect::from(crop_rect);
        let bogus = Rect::from(bogus_rect);

        let offset_filter = offset((-10.0, -10.0), None, None);
        let cf_offset = color_filter(cf.clone(), offset_filter, None);

        // These are composed with an outer erode along the other axis, so don't add a cropRect to
        // them or it will interfere with the second filter evaluation.
        let erode_x = erode((8.0, 0.0), None, None);
        let erode_y = erode((0.0, 8.0), None, None);

        let filters: [Option<ImageFilter>; 14] = [
            None,
            color_filter(cf.clone(), None, Some(crop)),
            blur(0.0, 0.0, TileMode::Decal, None, Some(crop)),
            blur(1.0, 1.0, TileMode::Decal, None, Some(crop)),
            blur(8.0, 0.0, TileMode::Decal, None, Some(crop)),
            blur(0.0, 8.0, TileMode::Decal, None, Some(crop)),
            blur(8.0, 8.0, TileMode::Decal, None, Some(crop)),
            erode((1.0, 1.0), None, Some(crop)),
            erode((8.0, 0.0), erode_y, Some(crop)),
            erode((0.0, 8.0), erode_x, Some(crop)),
            erode((8.0, 8.0), None, Some(crop)),
            merge(&[None, cf_offset], Some(crop)),
            blur(8.0, 8.0, TileMode::Decal, None, Some(bogus)),
            color_filter(cf, None, Some(bogus)),
        ];

        let r = Rect::from_wh(int_to_scalar(64), int_to_scalar(64));
        let margin = int_to_scalar(16);
        let dx = r.width() + margin;
        let dy = r.height() + margin;

        canvas.translate((margin, margin));
        for proc_fn in draw_proc {
            canvas.save();
            for filter in &filters {
                canvas.draw_image(&checkerboard, (0.0, 0.0), None);
                proc_fn(canvas, r, filter.clone());
                canvas.translate((0.0, dy));
            }
            canvas.restore();
            canvas.translate((dx, 0.0));
        }
    }
}

// Port of: gm/imagefilterscropped.cpp#L182 (chrome/m156), DEF_GM( return new ImageFiltersCroppedGM; )
crate::def_gm!(
    ImageFiltersCroppedGM,
    ImageFiltersCroppedGm { checkerboard: None }
);
