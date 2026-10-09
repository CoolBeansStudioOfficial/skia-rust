// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imageblurclampmode.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{int_to_scalar, make_surface};
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;

// Port of: gm/imageblurclampmode.cpp#L9-L22 (chrome/m156), make_image
fn make_image(canvas: &Canvas) -> Image {
    let info = ImageInfo::new_n32_premul((250, 200), None);
    let mut surface = make_surface(canvas, &info, None).expect("a surface");
    {
        let c = surface.canvas();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::BLUE);
        c.draw_rect(Rect::new(0.0, 0.0, 250.0, 200.0), &paint);
        paint.set_color(Color::GREEN);
        c.draw_circle((125.0, 100.0), 100.0, &paint);
        paint.set_color(Color::RED);
        c.draw_rect(Rect::new(0.0, 0.0, 80.0, 80.0), &paint);
    }
    surface.image_snapshot().expect("a snapshot")
}

// Port of: gm/imageblurclampmode.cpp#L24-L31 (chrome/m156), draw_image
fn draw_image(canvas: &Canvas, image: &Image, filter: Option<ImageFilter>) {
    let _acr = skia_rust_core::canvas::AutoCanvasRestore::guard(canvas, true);
    let mut paint = Paint::default();
    paint.set_image_filter(filter);
    canvas.translate((int_to_scalar(30), 0.0));
    canvas.clip_irect(image.bounds(), None);
    canvas.draw_image_with_sampling_options(
        image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&paint),
    );
}

// Port of: gm/imageblurclampmode.cpp#L33-L80 (chrome/m156), ImageBlurClampModeGM
struct ImageBlurClampModeGm;

impl GM for ImageBlurClampModeGm {
    fn name(&self) -> String {
        "imageblurclampmode".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(850, 920)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    // Port of: gm/imageblurclampmode.cpp#L44-L72 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let image = make_image(canvas);
        canvas.translate((0.0, 30.0));
        // Test different kernel size, including the one to launch 2d Gaussian blur.
        for sigma in [0.6_f32, 3.0, 8.0, 20.0] {
            canvas.save();
            // x-only blur
            let filter = blur(
                sigma,
                0.0,
                TileMode::Clamp,
                None,
                Some(Rect::from_irect(image.bounds())),
            );
            draw_image(canvas, &image, filter);
            canvas.translate((int_to_scalar(image.width()) + 20.0, 0.0));
            // y-only blur
            let filter = blur(
                0.0,
                sigma,
                TileMode::Clamp,
                None,
                Some(Rect::from_irect(image.bounds())),
            );
            draw_image(canvas, &image, filter);
            canvas.translate((int_to_scalar(image.width()) + 20.0, 0.0));
            // both directions
            let filter = blur(
                sigma,
                sigma,
                TileMode::Clamp,
                None,
                Some(Rect::from_irect(image.bounds())),
            );
            draw_image(canvas, &image, filter);
            canvas.translate((int_to_scalar(image.width()) + 20.0, 0.0));
            canvas.restore();
            canvas.translate((0.0, int_to_scalar(image.height()) + 20.0));
        }
    }
}

// Port of: gm/imageblurclampmode.cpp#L82 (chrome/m156)
crate::def_gm!(ImageBlurClampModeGM, ImageBlurClampModeGm);
