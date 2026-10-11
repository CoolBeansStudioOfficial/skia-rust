// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pictureimagefilter.cpp (chrome/m156)

// This GM exercises the SkPictureImageFilter ImageFilter class.

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar_round_to_int;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_effects::image_filters::{image, picture as picture_filter};
use skia_rust_raster::image_picture::{BitDepth, deferred_from_picture};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/pictureimagefilter.cpp#L10-L20 (chrome/m156), fill_rect_filtered
fn fill_rect_filtered(canvas: &Canvas, clip_rect: Rect, filter: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(filter);
    canvas.save();
    canvas.clip_rect(clip_rect, None, None);
    canvas.draw_paint(&paint);
    canvas.restore();
}

// Port of: gm/pictureimagefilter.cpp#L22-L31 (chrome/m156), make_picture
fn make_picture() -> Option<Picture> {
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::from_wh(100.0, 100.0), false);
    let mut paint = Paint::default();
    paint.set_color(Color::new(0xFFFF_FFFF));
    let font = Font::from_size(default_portable_typeface(), 96.0);
    canvas.draw_str("e", (20.0, 70.0), &font, &paint);
    recorder.finish_recording_as_picture(None)
}

// Create a picture that will draw LCD text
// Port of: gm/pictureimagefilter.cpp#L33-L44 (chrome/m156), make_LCD_picture
fn make_lcd_picture() -> Option<Picture> {
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::from_wh(100.0, 100.0), false);
    canvas.clear(Color::TRANSPARENT);
    let mut paint = Paint::default();
    paint.set_color(Color::new(0xFFFF_FFFF));
    // this has to be small enough that it doesn't become a path
    let mut font = Font::from_size(default_portable_typeface(), 36.0);
    font.set_edging(Edging::SubpixelAntiAlias);
    canvas.draw_str("e", (20.0, 70.0), &font, &paint);
    recorder.finish_recording_as_picture(None)
}

// Port of: gm/pictureimagefilter.cpp#L46-L49 (chrome/m156), the class's make()
fn make(pic: Option<Picture>, r: Rect, sampling: SamplingOptions) -> Option<ImageFilter> {
    let pic = pic?;
    let dim = (
        scalar_round_to_int(r.width()),
        scalar_round_to_int(r.height()),
    );
    let img = deferred_from_picture(
        pic,
        dim,
        None,
        None,
        BitDepth::U8,
        Some(ColorSpace::new_srgb()),
        SurfaceProps::default(),
    );
    Some(image(img, r, r, sampling))
}

// Port of: gm/pictureimagefilter.cpp#L97-L101 (chrome/m156), PictureImageFilterGM
struct PictureImageFilterGm {
    picture: Option<Picture>,
    lcd_picture: Option<Picture>,
}

impl PictureImageFilterGm {
    fn new() -> Self {
        Self {
            picture: None,
            lcd_picture: None,
        }
    }
}

impl GM for PictureImageFilterGm {
    fn name(&self) -> String {
        "pictureimagefilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(600, 300)
    }

    // Port of: gm/pictureimagefilter.cpp#L56-L59 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.picture = make_picture();
        self.lcd_picture = make_lcd_picture();
    }

    // Port of: gm/pictureimagefilter.cpp#L107-L162 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::GRAY);
        let Some(picture) = self.picture.clone() else {
            return;
        };
        let Some(lcd_picture) = self.lcd_picture.clone() else {
            return;
        };
        let src_rect = Rect::from_xywh(20.0, 20.0, 30.0, 30.0);
        let empty_rect = Rect::from_xywh(20.0, 20.0, 0.0, 0.0);
        let bounds = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        let picture_source = picture_filter(Some(picture.clone()), None);
        let picture_source_src_rect = picture_filter(Some(picture.clone()), Some(&src_rect));
        let picture_source_empty_rect = picture_filter(Some(picture.clone()), Some(&empty_rect));
        let picture_source_resampled = make(
            Some(picture.clone()),
            picture.cull_rect(),
            SamplingOptions::from(FilterMode::Linear),
        );
        let picture_source_pixelated = make(
            Some(picture.clone()),
            picture.cull_rect(),
            SamplingOptions::default(),
        );

        canvas.save();
        // Draw the picture unscaled.
        fill_rect_filtered(canvas, bounds, picture_source.clone());
        canvas.translate((int_to_scalar(100), 0.0));
        // Draw an unscaled subset of the source picture.
        fill_rect_filtered(canvas, bounds, picture_source_src_rect);
        canvas.translate((int_to_scalar(100), 0.0));
        // Draw the picture to an empty rect (should draw nothing).
        fill_rect_filtered(canvas, bounds, picture_source_empty_rect);
        canvas.translate((int_to_scalar(100), 0.0));
        // Draw the LCD picture to a layer
        {
            let mut stroke = Paint::default();
            stroke.set_style(Style::Stroke);
            canvas.draw_rect(bounds, &stroke);
            let mut paint = Paint::default();
            paint.set_image_filter(make(
                Some(lcd_picture),
                picture.cull_rect(),
                SamplingOptions::default(),
            ));
            canvas.scale((4.0, 4.0));
            canvas.translate((-0.9_f32 * src_rect.left, -2.45_f32 * src_rect.top));
            canvas.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&paint));
            canvas.restore();
        }
        canvas.restore();

        // Draw the picture scaled
        canvas.translate((0.0, int_to_scalar(100)));
        canvas.scale((200.0 / src_rect.width(), 200.0 / src_rect.height()));
        canvas.translate((-src_rect.left, -src_rect.top));
        fill_rect_filtered(canvas, src_rect, picture_source);

        // Draw the picture scaled, but rasterized at original resolution
        canvas.translate((src_rect.width(), 0.0));
        fill_rect_filtered(canvas, src_rect, picture_source_resampled);

        // Draw the picture scaled, pixelated
        canvas.translate((src_rect.width(), 0.0));
        fill_rect_filtered(canvas, src_rect, picture_source_pixelated);
    }
}

// Port of: gm/pictureimagefilter.cpp#L159 (chrome/m156), DEF_GM( return new PictureImageFilterGM; )
crate::def_gm!(PictureImageFilterGM, PictureImageFilterGm::new());
