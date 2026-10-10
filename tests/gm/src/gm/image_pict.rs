// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/image_pict.cpp (chrome/m156), ImagePictGM and the ImageCacheratorGM picture and
// raster variants
//
// Not ported: the texture variant of `ImageCacheratorGM` (GPU-only: its generator needs a GPU
// context, so the raster sink skips it).

use crate::canvas::Canvas;
use crate::{DrawResult, GM};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_base::NEED_NEW_IMAGE_UNIQUE_ID;
use skia_rust_core::image_generator::{ImageGenerator, generator_unique_id};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_raster::image_picture::{BitDepth, deferred_from_picture, make_from_picture};
use skia_rust_raster::raster_canvas::RasterCanvas;

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

// Port of: gm/image_pict.cpp#L136-L140 (chrome/m156), draw_placeholder
#[allow(clippy::many_single_char_names)] // mirrors the C++ parameter names
#[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar of small sizes (exact in f32)
fn draw_placeholder(canvas: &Canvas, x: f32, y: f32, w: i32, h: i32) {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    let r = Rect::from_xywh(x, y, w as f32, h as f32);
    canvas.draw_rect(r, &paint);
    canvas.draw_line((r.left(), r.top()), (r.right(), r.bottom()), &paint);
    canvas.draw_line((r.left(), r.bottom()), (r.right(), r.top()), &paint);
}

// Port of: gm/image_pict.cpp#L350-L358 (chrome/m156), draw_as_bitmap
fn draw_as_bitmap(canvas: &Canvas, image: &Image, x: f32, y: f32) {
    if let Some(bitmap) = image.get_ro_pixels() {
        if let Some(bitmap_image) = bitmap.as_image() {
            canvas.draw_image(&bitmap_image, (x, y), None);
        }
    } else {
        draw_placeholder(canvas, x, y, image.width(), image.height());
    }
}

// Port of: gm/image_pict.cpp#L360-L385 (chrome/m156), draw_as_tex (the CPU branch: no GPU view)
fn draw_as_tex(canvas: &Canvas, image: &Image, x: f32, y: f32) {
    canvas.draw_image(image, (x, y), None);
}

// Port of: gm/image_pict.cpp#L127-L153 (chrome/m156), class RasterGenerator
struct RasterGenerator {
    info: ImageInfo,
    unique_id: u32,
    bm: Bitmap,
}

impl ImageGenerator for RasterGenerator {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    // Port of: gm/image_pict.cpp#L137-L146 (chrome/m156), RasterGenerator::onGetPixels
    fn on_get_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        self.bm.read_pixels(info, pixels, row_bytes, 0, 0)
    }
}

// Port of: gm/image_pict.cpp#L164-L173 (chrome/m156), make_ras_generator
fn make_ras_generator(pic: &Picture) -> Option<Box<dyn ImageGenerator>> {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((100, 100), None);
    {
        let canvas = <skia_rust_core::canvas::Canvas as RasterCanvas>::from_bitmap(&mut bm, None)?;
        canvas.clear(Color::TRANSPARENT);
        canvas.translate((-100.0, -100.0));
        canvas.draw_picture(pic, None, None);
    }
    let info = bm.info().clone();
    Some(Box::new(RasterGenerator {
        info,
        unique_id: generator_unique_id(NEED_NEW_IMAGE_UNIQUE_ID),
        bm,
    }))
}

// Port of: gm/image_pict.cpp#L175-L186 (chrome/m156), make_pic_generator
fn make_pic_generator(pic: &Picture) -> Option<Box<dyn ImageGenerator>> {
    let matrix = Matrix::translate((-100.0, -100.0));
    make_from_picture(
        (100, 100),
        pic.clone(),
        Some(&matrix),
        None,
        BitDepth::U8,
        Some(ColorSpace::new_srgb()),
        SurfaceProps::default(),
    )
}

/// The generator factory of an `ImageCacheratorGM` (`FactoryFunc`).
type FactoryFunc = fn(&Picture) -> Option<Box<dyn ImageGenerator>>;

// Port of: gm/image_pict.cpp#L257-L366 (chrome/m156), class ImageCacheratorGM (the picture and
// raster generators; the texture generator is GPU-only and not ported)
struct ImageCacheratorGm {
    name: String,
    factory: FactoryFunc,
    picture: Option<Picture>,
    image: Option<Image>,
    image_subset: Option<Image>,
}

impl ImageCacheratorGm {
    // Port of: gm/image_pict.cpp#L285-L324 (chrome/m156), makeCaches
    fn make_caches(&mut self) -> bool {
        let Some(picture) = self.picture.clone() else {
            return false;
        };
        {
            let Some(generator) = (self.factory)(&picture) else {
                return false;
            };
            self.image = images::deferred_from_generator(Some(generator));
            if self.image.is_none() {
                return false;
            }
        }

        {
            let subset = IRect::from_ltrb(50, 50, 100, 100);

            // We re-create the generator here on the off chance that making a subset from
            // 'fImage' might perturb its state.
            let Some(generator) = (self.factory)(&picture) else {
                return false;
            };
            // `canvas->baseRecorder()` is null on the raster sink, so the subset is made without a
            // recorder.
            self.image_subset = images::deferred_from_generator(Some(generator))
                .and_then(|image| image.make_subset(subset, RequiredProperties::default()));
            if self.image_subset.is_none() {
                return false;
            }
        }

        true
    }

    // Port of: gm/image_pict.cpp#L388-L405 (chrome/m156), drawRow
    fn draw_row(&self, canvas: &Canvas, scale: f32) {
        canvas.scale((scale, scale));

        let matrix = Matrix::translate((-100.0, -100.0));
        if let Some(picture) = &self.picture {
            canvas.draw_picture(picture, Some(&matrix), None);
        }

        // Draw the tex first, so it doesn't hit a lucky cache from the raster version. This
        // way we also can force the generateTexture call.
        if let (Some(image), Some(image_subset)) = (&self.image, &self.image_subset) {
            draw_as_tex(canvas, image, 150.0, 0.0);
            draw_as_tex(canvas, image_subset, 150.0 + 101.0, 0.0);

            draw_as_bitmap(canvas, image, 310.0, 0.0);
            draw_as_bitmap(canvas, image_subset, 310.0 + 101.0, 0.0);
        }
    }
}

impl GM for ImageCacheratorGm {
    // Port of: gm/image_pict.cpp#L270-L274 (chrome/m156), getName
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: gm/image_pict.cpp#L276-L276 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(960, 450)
    }

    // Port of: gm/image_pict.cpp#L278-L283 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let bounds = Rect::from_xywh(100.0, 100.0, 100.0, 100.0);
        let mut recorder = PictureRecorder::new();
        draw_something(recorder.begin_recording(bounds, false), bounds);
        self.picture = recorder.finish_recording_as_picture(None);
    }

    // Port of: gm/image_pict.cpp#L407-L435 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        if !self.make_caches() {
            *error_msg = "Could not create cached images".to_string();
            return DrawResult::Skip;
        }

        canvas.save();
        canvas.translate((20.0, 20.0));
        self.draw_row(canvas, 1.0);
        canvas.restore();

        canvas.save();
        canvas.translate((20.0, 150.0));
        self.draw_row(canvas, 0.25);
        canvas.restore();

        canvas.save();
        canvas.translate((20.0, 220.0));
        self.draw_row(canvas, 2.0);
        canvas.restore();

        DrawResult::Ok
    }
}

// Port of: gm/image_pict.cpp#L441 (chrome/m156), DEF_GM( return new ImageCacheratorGM("raster", ...) )
crate::def_gm!(
    ImageCacheratorGM_raster = "ImageCacheratorGM(\"raster\", make_ras_generator, false)",
    ImageCacheratorGm {
        name: "image-cacherator-from-raster".to_string(),
        factory: make_ras_generator,
        picture: None,
        image: None,
        image_subset: None,
    }
);

// Port of: gm/image_pict.cpp#L440 (chrome/m156), DEF_GM( return new ImageCacheratorGM("picture", ...) )
crate::def_gm!(
    ImageCacheratorGM_picture = "ImageCacheratorGM(\"picture\", make_pic_generator, false)",
    ImageCacheratorGm {
        name: "image-cacherator-from-picture".to_string(),
        factory: make_pic_generator,
        picture: None,
        image: None,
        image_subset: None,
    }
);
