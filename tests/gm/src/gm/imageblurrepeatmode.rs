// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imageblurrepeatmode.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts and loop indices.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::similar_names
)]

use crate::prelude::*;
use crate::tool_utils::{create_checkerboard_image, int_to_scalar, make_surface};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, crop};

// Port of: gm/imageblurrepeatmode.cpp#L9-L44 (chrome/m156), make_image
fn make_image(canvas: &Canvas, direction: i32) -> Image {
    let info = ImageInfo::new_n32_premul((250, 200), None);
    let mut surface = make_surface(canvas, &info, None).expect("a surface");
    {
        let c = surface.canvas();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let colors = [
            Color::RED,
            Color::BLUE,
            Color::GREEN,
            Color::YELLOW,
            Color::BLACK,
        ];
        let width: i32 = 25;
        let x_direction = (direction & 0x1) == 1;
        let y_direction = (direction & 0x2) == 2;
        if x_direction {
            let mut x = 0;
            while x < info.width() {
                paint.set_color(colors[(x / width % 5) as usize]);
                if y_direction {
                    paint.set_alpha_f(0.5);
                }
                c.draw_rect(
                    Rect::new(
                        int_to_scalar(x),
                        0.0,
                        int_to_scalar(x + width),
                        int_to_scalar(info.height()),
                    ),
                    &paint,
                );
                x += width;
            }
        }
        if y_direction {
            let mut y = 0;
            while y < info.height() {
                paint.set_color(colors[(y / width % 5) as usize]);
                if x_direction {
                    paint.set_alpha_f(0.5);
                }
                c.draw_rect(
                    Rect::new(
                        0.0,
                        int_to_scalar(y),
                        int_to_scalar(info.width()),
                        int_to_scalar(y + width),
                    ),
                    &paint,
                );
                y += width;
            }
        }
    }
    surface.image_snapshot().expect("a snapshot")
}

// Port of: gm/imageblurrepeatmode.cpp#L46-L53 (chrome/m156), draw_image
fn draw_image(canvas: &Canvas, image: &Image, filter: Option<ImageFilter>) {
    let _acr = AutoCanvasRestore::guard(canvas, true);
    let mut paint = Paint::default();
    paint.set_image_filter(filter);
    canvas.translate((30.0, 0.0));
    canvas.clip_irect(image.bounds(), None);
    canvas.draw_image_with_sampling_options(
        image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&paint),
    );
}

// Port of: gm/imageblurrepeatmode.cpp#L55-L92 (chrome/m156), ImageBlurRepeatModeGM
struct ImageBlurRepeatModeGm;

impl GM for ImageBlurRepeatModeGm {
    fn name(&self) -> String {
        "imageblurrepeatmode".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(850, 920)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    // Port of: gm/imageblurrepeatmode.cpp#L63-L89 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let image = [
            make_image(canvas, 1),
            make_image(canvas, 2),
            make_image(canvas, 3),
        ];
        canvas.translate((0.0, 30.0));
        // Test different kernel size, including the one to launch 2d Gaussian blur.
        for sigma in [0.6_f32, 3.0, 8.0, 20.0] {
            // FIXME crops
            canvas.save();
            let filter = blur(
                sigma,
                0.0,
                TileMode::Repeat,
                None,
                Some(Rect::from_irect(image[0].bounds())),
            );
            draw_image(canvas, &image[0], filter);
            canvas.translate((int_to_scalar(image[0].width()) + 20.0, 0.0));
            let filter = blur(
                0.0,
                sigma,
                TileMode::Repeat,
                None,
                Some(Rect::from_irect(image[1].bounds())),
            );
            draw_image(canvas, &image[1], filter);
            canvas.translate((int_to_scalar(image[1].width()) + 20.0, 0.0));
            let filter = blur(
                sigma,
                sigma,
                TileMode::Repeat,
                None,
                Some(Rect::from_irect(image[2].bounds())),
            );
            draw_image(canvas, &image[2], filter);
            canvas.translate((int_to_scalar(image[2].width()) + 20.0, 0.0));
            canvas.restore();
            canvas.translate((0.0, int_to_scalar(image[0].height()) + 20.0));
        }
    }
}

// Port of: gm/imageblurrepeatmode.cpp#L93 (chrome/m156)
crate::def_gm!(ImageBlurRepeatModeGM, ImageBlurRepeatModeGm);

// Port of: gm/imageblurrepeatmode.cpp#L94-L129 (chrome/m156), imageblurrepeatunclipped
crate::def_simple_gm!(imageblurrepeatunclipped, canvas, 256, 128, {
    // To show translucency
    let checkerboard = create_checkerboard_image(256, 128, Color::LIGHT_GRAY, Color::GRAY, 8);
    canvas.draw_image(&checkerboard, (0.0, 0.0), None);

    // Make an image with one red and one blue band
    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels((100, 20), None);
    bmp.erase(Color::RED, IRect::from_wh(100, 10));
    bmp.erase(Color::BLUE, IRect::from_xywh(0, 10, 100, 10));
    let img = bmp.as_image().expect("an image");

    // The blur filter uses a repeat crop applied to the image bounds to define the tiling geometry,
    // but the crop IF is created directly since the tilemode factory for ::Blur also adds a kDecal
    // post-crop that is undesired for this GM.
    let repeat_crop = crop(&Rect::from_irect(img.bounds()), TileMode::Repeat, None);
    let filter = blur(0.0, 10.0, TileMode::Decal, repeat_crop, None);
    let mut paint = Paint::default();
    paint.set_image_filter(filter);

    // Draw the blurred image once with a clip that shows the repeat is tiled several times.
    // 3xsigma is used to match the historic, but underspecified behavior for when kRepeat was used
    // with no crop rect (which must now be provided or kRepeat is ignored).
    canvas.translate((0.0, 50.0));
    canvas.save();
    let mut outset = img.bounds();
    outset.outset((0, 30));
    canvas.clip_irect(outset, None);
    canvas.draw_image_with_sampling_options(
        &img,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    canvas.restore();

    // Draw the blurred image with a clip positioned such that the draw would be excluded except
    // that the image filter causes it to intersect with the clip. It should look like the
    // left image, but clipped to the debug-black rectangle.
    canvas.translate((110.0, 0.0));
    canvas.save();
    canvas.clip_irect(IRect::from_xywh(0, -30, 100, 10), None);
    canvas.draw_image_with_sampling_options(
        &img,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    canvas.restore();

    // Visualize the clip
    let mut line = Paint::default();
    line.set_style(Style::Stroke);
    canvas.draw_rect(Rect::from_xywh(0.0, -30.0, 99.0, 9.0), &line);
});
