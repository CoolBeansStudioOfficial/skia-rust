// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmappremul.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::raster_canvas::RasterCanvas;

// This GM checks that bitmap pixels are unpremultiplied before being exported
// to other formats. If unpremultiplication is implemented properly, this
// GM should come out completely white. If not, this GM looks like a row of two
// greyscale gradients above a row of grey lines.
// This tests both the ARGB4444 and ARGB8888 bitmap configurations.

// Port of: gm/bitmappremul.cpp#L27 (chrome/m156)
const SLIDE_SIZE: i32 = 256;

// Port of: gm/bitmappremul.cpp#L29-L33 (chrome/m156)
fn init_bitmap(ct: ColorType, bitmap: &mut Bitmap) {
    bitmap.alloc_pixels_info(
        &ImageInfo::new((SLIDE_SIZE, SLIDE_SIZE), ct, AlphaType::Premul, None),
        None,
    );
    bitmap.erase_color(Color::WHITE);
}

// Port of: gm/bitmappremul.cpp#L35-L45 (chrome/m156)
fn make_argb8888_gradient() -> Option<Image> {
    let mut bitmap = Bitmap::new();
    init_bitmap(ColorType::N32, &mut bitmap);
    for y in 0..SLIDE_SIZE {
        let v = u32::try_from(y).expect("y");
        for x in 0..SLIDE_SIZE {
            bitmap.set_addr32(x, y, pack_argb32(v, v, v, v));
        }
    }
    bitmap.as_image()
}

// Port of: gm/bitmappremul.cpp#L47-L56 (chrome/m156)
fn make_argb4444_gradient() -> Option<Image> {
    let mut bitmap = Bitmap::new();
    init_bitmap(ColorType::ARGB4444, &mut bitmap);
    // Using draw rather than readPixels to suppress dither
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bitmap, None).expect("a canvas on the bitmap");
        canvas.draw_image_with_sampling_options(
            make_argb8888_gradient().expect("an image"),
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&paint),
        );
    }
    bitmap.as_image()
}

// Port of: gm/bitmappremul.cpp#L58-L74 (chrome/m156)
fn make_argb8888_stripes() -> Option<Image> {
    let mut bitmap = Bitmap::new();
    init_bitmap(ColorType::N32, &mut bitmap);
    let mut row_color: u8 = 0;
    for y in 0..SLIDE_SIZE {
        let v = u32::from(row_color);
        for x in 0..SLIDE_SIZE {
            bitmap.set_addr32(x, y, pack_argb32(v, v, v, v));
        }
        if row_color == 0 {
            row_color = 255;
        } else {
            row_color = 0;
        }
    }
    bitmap.as_image()
}

// Port of: gm/bitmappremul.cpp#L76-L85 (chrome/m156)
fn make_argb4444_stripes() -> Option<Image> {
    let mut bitmap = Bitmap::new();
    init_bitmap(ColorType::ARGB4444, &mut bitmap);
    // Using draw rather than readPixels to suppress dither
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bitmap, None).expect("a canvas on the bitmap");
        canvas.draw_image_with_sampling_options(
            make_argb8888_stripes().expect("an image"),
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&paint),
        );
    }
    bitmap.as_image()
}

// Port of: gm/bitmappremul.cpp#L89-L112 (chrome/m156)
struct BitmapPremulGm;

impl GM for BitmapPremulGm {
    fn name(&self) -> String {
        "bitmap_premul".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(SLIDE_SIZE * 2, SLIDE_SIZE * 2)
    }

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    fn on_draw(&mut self, canvas: &Canvas) {
        let slide_size = SLIDE_SIZE as scalar;
        canvas.draw_image(
            make_argb8888_gradient().expect("an image"),
            (0.0, 0.0),
            None,
        );
        canvas.draw_image(
            make_argb4444_gradient().expect("an image"),
            (slide_size, 0.0),
            None,
        );
        canvas.draw_image(
            make_argb8888_stripes().expect("an image"),
            (0.0, slide_size),
            None,
        );
        canvas.draw_image(
            make_argb4444_stripes().expect("an image"),
            (slide_size, slide_size),
            None,
        );
    }
}

// Port of: gm/bitmappremul.cpp#L114 (chrome/m156)
crate::def_gm!(BitmapPremulGM, BitmapPremulGm);

// Port of: gm/bitmappremul.cpp#L117-L118 (chrome/m156)
const BOX_SIZE: i32 = 31;
const PADDING: i32 = 5;

// Port of: gm/bitmappremul.cpp#L120-L131 (chrome/m156)
fn make_out_of_gamut_image(ct: ColorType) -> Option<Image> {
    let mut bmp = Bitmap::new();
    // Odd dimensions so that we hit the different implementation in the SIMD tail handling
    bmp.alloc_pixels_info(
        &ImageInfo::new((BOX_SIZE, BOX_SIZE), ct, AlphaType::Premul, None),
        None,
    );
    for y in 0..BOX_SIZE {
        for x in 0..BOX_SIZE {
            bmp.set_addr32(
                x,
                y,
                0x4000_0000
                    | (u32::try_from(x * 8).expect("x") << 8)
                    | u32::try_from(y * 8).expect("y"),
            );
        }
    }
    bmp.as_image()
}

// Port of: gm/bitmappremul.cpp#L133-L152 (chrome/m156)
crate::def_simple_gm!(
    image_out_of_gamut,
    canvas,
    2 * BOX_SIZE + 3 * PADDING,
    BOX_SIZE + 2 * PADDING,
    {
        // This GM draws an image with out-of-gamut colors (RGB > A). Historically, Skia assumed
        // this was impossible, and contained numerous asserts and optimizations that would break
        // if the rule were violated. With color spaces and/or SkSL shaders (among other things),
        // it's no longer reasonable to make this claim. To catch issues with legacy blitters,
        // this draws both RGBA and BGRA. (This ensures that we always hit the N32 -> N32 case).
        canvas.clear(Color::GRAY);

        let rgba = make_out_of_gamut_image(ColorType::RGBA8888).expect("an image");
        let bgra = make_out_of_gamut_image(ColorType::BGRA8888).expect("an image");

        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        {
            canvas.translate((PADDING as scalar, PADDING as scalar));
            canvas.draw_image(&rgba, (0.0, 0.0), None);
            canvas.translate(((BOX_SIZE + PADDING) as scalar, 0.0));
            canvas.draw_image(&bgra, (0.0, 0.0), None);
        }
    }
);
