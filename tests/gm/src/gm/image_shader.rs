// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/image_shader.cpp (chrome/m156)
//
// Not ported: `textureimage_and_shader` (`SkCanvas::getSurface`).

use crate::prelude::*;
use crate::tool_utils;
use skia_rust_codec::encode::png_encoder;
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::shaders::ImageShader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::image_picture::{BitDepth, deferred_from_picture};
use skia_rust_raster::surfaces;

// Port of: gm/image_shader.cpp#L174-L178 (chrome/m156)
fn make_checker_img(w: i32, h: i32, c0: Color, c1: Color, size: i32) -> Option<Image> {
    let mut bm = tool_utils::create_checkerboard_bitmap(w, h, c0, c1, size);
    bm.set_immutable();
    bm.as_image()
}

// Port of: gm/image_shader.cpp#L180-L218 (chrome/m156)
crate::def_simple_gm!(drawimage_sampling, canvas, 500, 500, {
    const N: i32 = 256;
    const SCALE: f32 = 1.0 / 6.0;
    #[allow(clippy::cast_precision_loss)] // kScale*N
    let dst = Rect::new(0.0, 0.0, SCALE * N as f32, SCALE * N as f32);

    let img = make_checker_img(N, N, Color::BLACK, Color::WHITE, 7)
        .expect("an image")
        .with_default_mipmaps()
        .expect("an image");
    #[allow(clippy::cast_precision_loss)] // SkRect::MakeIWH
    let src = Rect::from_wh(img.width() as f32, img.height() as f32);

    let mx = Matrix::rect_to_rect_or_identity(src, dst, None);

    let mut paint = Paint::default();

    for mm in [MipmapMode::None, MipmapMode::Nearest, MipmapMode::Linear] {
        for fm in [FilterMode::Nearest, FilterMode::Linear] {
            let sampling = SamplingOptions::new(fm, mm);

            canvas.save();

            canvas.save();
            canvas.concat(&mx);
            canvas.draw_image_with_sampling_options(&img, (0.0, 0.0), sampling, None);
            canvas.restore();

            canvas.translate((dst.width() + 4.0, 0.0));

            paint.set_shader(img.to_shader((TileMode::Clamp, TileMode::Clamp), sampling, &mx));
            canvas.draw_rect(dst, &paint);

            canvas.translate((dst.width() + 4.0, 0.0));

            // (`paint = nullptr` is the default paint.)
            canvas.draw_image_rect_with_sampling_options(
                &img,
                Some((&src, SrcRectConstraint::Fast)),
                dst,
                sampling,
                &Paint::default(),
            );
            canvas.restore();

            canvas.translate((0.0, dst.height() + 8.0));
        }
    }
});

// Port of: gm/image_shader.cpp#L254-L271 (chrome/m156)
crate::def_simple_gm!(imageshader_tinyscale, canvas, 1000, 1000, {
    // A small scale amplifies the sampling coords in image space.
    const K_SCALE: f32 = 0.01;

    // 128x128px image with red/black/green/blue quadrants.
    let img = tool_utils::get_resource_as_image("images/gainmap_gcontainer_only.jpg")
        .expect("images/gainmap_gcontainer_only.jpg");

    let m = Matrix::translate((500.0, 500.0)) * Matrix::scale((K_SCALE, K_SCALE));

    // In clamp mode we should see no repeating patterns, just the viewport filled
    // with four-colored quadrants.
    let mut p = Paint::default();
    p.set_shader(img.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        &m,
    ));
    canvas.draw_paint(&p);
});

// Draws a red stroked rect and a blue oval, as the GM's picture content.
// Port of: gm/image_shader.cpp#L42-L52 (chrome/m156), draw_something
fn draw_something(canvas: &Canvas, bounds: &Rect) {
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

// The image makers of `gProcs`: each gives an image of the picture, or None.
type ImageMakerProc = fn(&Picture, &ImageInfo) -> Option<Image>;

// Port of: gm/image_shader.cpp#L56-L63 (chrome/m156), make_raster
fn make_raster(picture: &Picture, info: &ImageInfo) -> Option<Image> {
    let mut surface = surfaces::raster(info, None, None)?;
    surface.shared_canvas().clear(Color::TRANSPARENT);
    surface.shared_canvas().draw_picture(picture, None, None);
    surface.image_snapshot()
}

// The GPU variant: without a GPU context the C++ returns null, so the GM skips it on the raster
// sink (`make_texture`, Port of: gm/image_shader.cpp#L65-L81 (chrome/m156)).
fn make_texture(_picture: &Picture, _info: &ImageInfo) -> Option<Image> {
    None
}

// Port of: gm/image_shader.cpp#L83-L92 (chrome/m156), make_pict_gen
fn make_pict_gen(picture: &Picture, info: &ImageInfo) -> Option<Image> {
    deferred_from_picture(
        picture.clone(),
        info.dimensions(),
        None,
        None,
        BitDepth::U8,
        Some(ColorSpace::new_srgb()),
        SurfaceProps::default(),
    )
}

// Port of: gm/image_shader.cpp#L94-L106 (chrome/m156), make_encode_gen
fn make_encode_gen(picture: &Picture, info: &ImageInfo) -> Option<Image> {
    let src = make_raster(picture, info)?;
    let encoded = png_encoder::encode_image(&src, &png_encoder::Options::default())?;
    deferred_from_encoded_data(Some(encoded), None)
}

// Port of: gm/image_shader.cpp#L108-L114 (chrome/m156), gProcs
const G_PROCS: [ImageMakerProc; 4] = [make_raster, make_texture, make_pict_gen, make_encode_gen];

// Exercise drawing pictures inside an image, showing that the image version is pixelated
// (correctly) when it is inside an image.
// Port of: gm/image_shader.cpp#L119-L167 (chrome/m156), ImageShaderGM
struct ImageShaderGm {
    picture: Option<Picture>,
}

impl ImageShaderGm {
    // Port of: gm/image_shader.cpp#L137-L149 (chrome/m156), testImage
    fn test_image(canvas: &Canvas, image: &Image) {
        let save_count = canvas.save_count();
        canvas.save();
        canvas.draw_image(image, (0.0, 0.0), None);
        canvas.translate((0.0, 120.0));
        let tile = TileMode::Repeat;
        let local_m = Matrix::translate((-50.0, -50.0));
        let mut paint = Paint::default();
        paint.set_shader(ImageShader::make(
            Some(image.clone()),
            tile,
            tile,
            &SamplingOptions::default(),
            Some(&local_m),
            false,
        ));
        paint.set_anti_alias(true);
        canvas.draw_circle((50.0, 50.0), 50.0, &paint);
        canvas.restore_to_count(save_count);
    }
}

impl GM for ImageShaderGm {
    fn name(&self) -> String {
        "image-shader".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(850, 450)
    }

    // Port of: gm/image_shader.cpp#L130-L135 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let bounds = Rect::from_wh(100.0, 100.0);
        let mut recorder = PictureRecorder::new();
        draw_something(recorder.begin_recording(bounds, false), &bounds);
        self.picture = recorder.finish_recording_as_picture(None);
    }

    // Port of: gm/image_shader.cpp#L151-L165 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((20.0, 20.0));
        let info = ImageInfo::new_n32_premul((100, 100), None);
        let Some(picture) = self.picture.as_ref() else {
            return;
        };
        for make in G_PROCS {
            if let Some(image) = make(picture, &info) {
                Self::test_image(canvas, &image);
            }
            canvas.translate((120.0, 0.0));
        }
    }
}

crate::def_gm!(ImageShaderGM, ImageShaderGm { picture: None });
