// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmapshader.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/bitmapshader.cpp#L30-L38 (chrome/m156)
fn draw_bm() -> Option<Image> {
    let mut blue_paint = Paint::default();
    blue_paint.set_color(Color::BLUE);

    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((20, 20), None);
    bm.erase_color(Color::RED);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
        canvas.draw_circle((10.0, 10.0), 5.0, &blue_paint);
    }
    bm.as_image()
}

// Port of: gm/bitmapshader.cpp#L40-L48 (chrome/m156)
fn draw_mask() -> Option<Image> {
    let mut circle_paint = Paint::default();
    circle_paint.set_color(Color::BLACK);

    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&ImageInfo::new_a8((20, 20)), None);
    bm.erase_color(Color::TRANSPARENT);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
        canvas.draw_circle((10.0, 10.0), 10.0, &circle_paint);
    }
    bm.as_image()
}

// Port of: gm/bitmapshader.cpp#L50-L116 (chrome/m156)
struct BitmapShaderGm {
    image: Option<Image>,
    mask: Option<Image>,
    bg_color: Color,
}

impl BitmapShaderGm {
    fn new() -> BitmapShaderGm {
        BitmapShaderGm {
            image: None,
            mask: None,
            bg_color: Color::WHITE,
        }
    }
}

impl GM for BitmapShaderGm {
    fn name(&self) -> String {
        "bitmapshaders".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(150, 100)
    }

    fn bg_color(&self) -> Color {
        self.bg_color
    }

    fn on_once_before_draw(&mut self) {
        self.bg_color = Color::GRAY;
        self.image = draw_bm();
        self.mask = draw_mask();
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        let image = self.image.as_ref().expect("image");
        let mask = self.mask.as_ref().expect("mask");

        for i in 0..2 {
            let mut s = Matrix::new_identity();
            s.reset();
            if 1 == i {
                s.set_scale((1.5, 1.5), None);
                s.post_translate((2.0, 2.0));
            }

            canvas.save();
            paint.set_shader(image.to_shader(
                None::<(TileMode, TileMode)>,
                SamplingOptions::default(),
                &s,
            ));

            // draw the shader with a bitmap mask
            canvas.draw_image_with_sampling_options(
                mask,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
            // no blue circle expected (the bitmap shader's coordinates are aligned to CTM still)
            canvas.draw_image_with_sampling_options(
                mask,
                (30.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );

            canvas.translate((0.0, 25.0));

            canvas.draw_circle((10.0, 10.0), 10.0, &paint);
            canvas.draw_circle((40.0, 10.0), 10.0, &paint); // no blue circle expected

            canvas.translate((0.0, 25.0));

            // clear the shader, colorized by a solid color with a bitmap mask
            paint.set_shader(None);
            paint.set_color(Color::GREEN);
            canvas.draw_image_with_sampling_options(
                mask,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
            canvas.draw_image_with_sampling_options(
                mask,
                (30.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );

            canvas.translate((0.0, 25.0));

            paint.set_shader(mask.to_shader(
                (TileMode::Repeat, TileMode::Repeat),
                SamplingOptions::default(),
                &s,
            ));
            paint.set_color(Color::RED);

            // draw the mask using the shader and a color
            canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 20.0, 20.0), &paint);
            canvas.draw_rect(Rect::from_xywh(30.0, 0.0, 20.0, 20.0), &paint);
            canvas.restore();
            canvas.translate((60.0, 0.0));
        }
    }
}

// Port of: gm/bitmapshader.cpp#L118-L146 (chrome/m156)
crate::def_simple_gm!(hugebitmapshader, canvas, 100, 100, {
    let mut paint = Paint::default();
    let mut bitmap = Bitmap::new();

    // The huge height will exceed GL_MAX_TEXTURE_SIZE. We test that the GL backend will at least
    // draw something with a default paint instead of drawing nothing.
    //
    // (See https://skia-review.googlesource.com/c/skia/+/73200)
    let bitmap_w = 1;
    let bitmap_h = 60000;
    let pixels: Vec<u8> = (0..bitmap_h)
        .map(|i: i32| u8::try_from(i & 0xff).expect("masked to 8 bits"))
        .collect();
    let installed = bitmap.install_pixels(
        &ImageInfo::new_a8((bitmap_w, bitmap_h)),
        pixels,
        usize::try_from(bitmap_w).expect("width"),
    );
    assert!(installed);

    paint.set_shader(bitmap.to_shader(
        (TileMode::Mirror, TileMode::Mirror),
        SamplingOptions::default(),
        None::<&Matrix>,
    ));
    paint.set_color(Color::RED);
    paint.set_anti_alias(true);
    canvas.draw_circle((50.0, 50.0), 50.0, &paint);
});

// Port of: gm/bitmapshader.cpp#L150 (chrome/m156)
crate::def_gm!(BitmapShaderGM, BitmapShaderGm::new());
