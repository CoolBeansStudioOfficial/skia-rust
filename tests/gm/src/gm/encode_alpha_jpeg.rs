// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/encode_alpha_jpeg.cpp (chrome/m156)

use skia_rust_codec::encode::jpeg_encoder::{self, AlphaOption, Options};
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;

/// Port of `read_into_pixmap` (encode_alpha_jpeg.cpp#L15-L20): the pixels of `src` in `info`, in a
/// new bitmap. `None` if the bitmap cannot be allocated.
fn read_into_bitmap(info: &ImageInfo, src: &Image) -> Option<Bitmap> {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(info, None);
    {
        let mut pm = bitmap.peek_pixels_mut()?;
        // The result is not checked, as in the C++ (`src->readPixels(...)`).
        let _ = src.read_pixels_to_pixmap(&mut pm, (0, 0));
    }
    Some(bitmap)
}

/// Port of `encode_pixmap_and_make_image` (encode_alpha_jpeg.cpp#L22-L29): the JPEG of `bitmap`
/// with the alpha option, decoded again as a lazy image.
fn encode_pixmap_and_make_image(bitmap: &Bitmap, alpha_option: AlphaOption) -> Option<Image> {
    let src = bitmap.peek_pixels()?;
    let options = Options {
        alpha_option,
        ..Options::default()
    };
    let data = jpeg_encoder::encode_pixmap(&src, &options)?;
    deferred_from_encoded_data(Some(data), None)
}

/// Port of `class EncodeJpegAlphaOptsGM` (encode_alpha_jpeg.cpp#L31-L105).
struct EncodeJpegAlphaOptsGm;

impl GM for EncodeJpegAlphaOptsGm {
    fn name(&self) -> String {
        "encode-alpha-jpeg".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(400, 200)
    }

    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(src_img) = get_resource_as_image("images/rainbow-gradient.png") else {
            *error_msg = "Could not load images/rainbow-gradient.png. \
                          Did you forget to set the resourcePath?"
                .to_string();
            return DrawResult::Fail;
        };
        let (w, h) = (src_img.width(), src_img.height());

        // SkImageInfo::MakeN32Premul(w, h, canvas->imageInfo().colorSpace() ? sRGB : nullptr).
        let srgb = canvas
            .image_info()
            .color_space()
            .map(|_| ColorSpace::new_srgb());
        let n32 = ImageInfo::new_n32_premul((w, h), srgb.clone());

        let draw = |canvas: &Canvas, bitmap: &Bitmap, x: f32, y: f32| {
            for (alpha_option, dy) in [
                (AlphaOption::Ignore, 0.0),
                (AlphaOption::BlendOnBlack, 100.0),
            ] {
                if let Some(image) = encode_pixmap_and_make_image(bitmap, alpha_option) {
                    canvas.draw_image(&image, (x, y + dy), None);
                }
            }
        };

        // Encode 8888 premul.
        if let Some(bitmap) = read_into_bitmap(&n32, &src_img) {
            draw(canvas, &bitmap, 0.0, 0.0);
        }

        // Encode 8888 unpremul.
        let n32_unpremul = n32.with_alpha_type(AlphaType::Unpremul);
        if let Some(bitmap) = read_into_bitmap(&n32_unpremul, &src_img) {
            draw(canvas, &bitmap, 100.0, 0.0);
        }

        // Encode F16 premul, in sRGB.
        let f16 = ImageInfo::new(
            (w, h),
            ColorType::RGBAF16,
            AlphaType::Premul,
            ColorSpace::new_srgb(),
        );
        if let Some(bitmap) = read_into_bitmap(&f16, &src_img) {
            draw(canvas, &bitmap, 200.0, 0.0);
        }

        // Encode F16 unpremul.
        let f16_unpremul = f16.with_alpha_type(AlphaType::Unpremul);
        if let Some(bitmap) = read_into_bitmap(&f16_unpremul, &src_img) {
            draw(canvas, &bitmap, 300.0, 0.0);
        }

        DrawResult::Ok
    }
}

// Port of: gm/encode_alpha_jpeg.cpp#L107 (chrome/m156)
crate::def_gm!(EncodeJpegAlphaOptsGM, EncodeJpegAlphaOptsGm);
