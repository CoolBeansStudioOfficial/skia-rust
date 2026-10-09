// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/image_pict.cpp (chrome/m156), ImagePictGM only
//
// Not ported: `ImageCacheratorGM` (needs `SkImageCacherator` and `makeSubset` of picture images)
// and its `picture`/`raster`/`texture` generators; the texture generator is GPU-only.

use crate::GM;
use crate::canvas::Canvas;
use crate::prelude::*;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_raster::image_picture::{BitDepth, deferred_from_picture};

// Port of: gm/image_pict.cpp#L31-L43 (chrome/m156), draw_something
fn draw_something(canvas: &Canvas, bounds: Rect) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::RED);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(10.0);
    canvas.draw_rect(bounds, &paint);
    paint.set_style(Style::Fill);
    paint.set_color(Color::BLUE);
    canvas.draw_oval(bounds, &paint);
}

// Port of: gm/image_pict.cpp#L45-L134 (chrome/m156), class ImagePictGM
struct ImagePictGm {
    picture: Option<Picture>,
    image0: Option<Image>,
    image1: Option<Image>,
}

impl ImagePictGm {
    // Port of: gm/image_pict.cpp#L104-L110 (chrome/m156), drawSet
    fn draw_set(&self, canvas: &Canvas) {
        let matrix = Matrix::translate((-100.0, -100.0));
        if let Some(picture) = &self.picture {
            canvas.draw_picture(picture, Some(&matrix), None);
        }
        if let Some(image0) = &self.image0 {
            canvas.draw_image(image0, (150.0, 0.0), None);
        }
        if let Some(image1) = &self.image1 {
            canvas.draw_image(image1, (300.0, 0.0), None);
        }
    }
}

impl GM for ImagePictGm {
    // Port of: gm/image_pict.cpp#L79-L79 (chrome/m156), getName
    fn name(&self) -> String {
        "image-picture".to_string()
    }

    // Port of: gm/image_pict.cpp#L81-L81 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(850, 450)
    }

    // Port of: gm/image_pict.cpp#L83-L102 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let bounds = Rect::from_xywh(100.0, 100.0, 100.0, 100.0);
        let mut recorder = PictureRecorder::new();
        draw_something(recorder.begin_recording(bounds, false), bounds);
        let picture = recorder.finish_recording_as_picture(None);

        // extract enough just for the oval.
        let size = ISize::new(100, 100);
        let srgb_color_space = ColorSpace::new_srgb();

        let mut matrix = Matrix::translate((-100.0, -100.0));
        self.image0 = picture.clone().and_then(|picture| {
            deferred_from_picture(
                picture,
                size,
                Some(&matrix),
                None,
                BitDepth::U8,
                Some(srgb_color_space.clone()),
                SurfaceProps::default(),
            )
        });
        matrix.post_translate((-50.0, -50.0));
        matrix.post_rotate(45.0, None);
        matrix.post_translate((50.0, 50.0));
        self.image1 = picture.clone().and_then(|picture| {
            deferred_from_picture(
                picture,
                size,
                Some(&matrix),
                None,
                BitDepth::U8,
                Some(srgb_color_space),
                SurfaceProps::default(),
            )
        });
        self.picture = picture;
    }

    // Port of: gm/image_pict.cpp#L112-L126 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((20.0, 20.0));

        self.draw_set(canvas);

        canvas.save();
        canvas.translate((0.0, 130.0));
        canvas.scale((0.25, 0.25));
        self.draw_set(canvas);
        canvas.restore();

        canvas.save();
        canvas.translate((0.0, 200.0));
        canvas.scale((2.0, 2.0));
        self.draw_set(canvas);
        canvas.restore();
    }
}

// Port of: gm/image_pict.cpp#L133 (chrome/m156), DEF_GM( return new ImagePictGM; )
crate::def_gm!(
    ImagePictGM,
    ImagePictGm {
        picture: None,
        image0: None,
        image1: None,
    }
);
