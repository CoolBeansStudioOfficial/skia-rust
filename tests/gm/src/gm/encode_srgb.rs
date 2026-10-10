// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/encode_srgb.cpp (chrome/m156). The WebP variant is not registered: the WebP encoder
// is not ported.

use skia_rust_codec::codecs::make_codec_from_stream;
use skia_rust_codec::encode::{jpeg_encoder, png_encoder};
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::stream::MemoryStream;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_data;

const IMAGE_WIDTH: i32 = 128;
const IMAGE_HEIGHT: i32 = 128;
/// `IMAGE_WIDTH` and `IMAGE_HEIGHT` as the `float` translations the GM uses.
const IMAGE_WIDTH_F: f32 = 128.0;
const IMAGE_HEIGHT_F: f32 = 128.0;

/// The encoded format of the GM (`SkEncodedImageFormat`), for the two encoders that are ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EncodedFormat {
    Png,
    Jpeg,
}

/// Port of `make(SkBitmap*, SkColorType, SkAlphaType, sk_sp<SkColorSpace>)` (encode_srgb.cpp#L19-L43):
/// decodes the resource into `bitmap` with the requested colour type, alpha type and colour space.
fn make(
    bitmap: &mut Bitmap,
    color_type: ColorType,
    alpha_type: AlphaType,
    color_space: Option<ColorSpace>,
) {
    let mut alpha_type = alpha_type;
    let resource = match color_type {
        ColorType::Gray8 => {
            alpha_type = AlphaType::Opaque;
            "images/grayscale.jpg"
        }
        ColorType::RGB565 => {
            alpha_type = AlphaType::Opaque;
            "images/color_wheel.jpg"
        }
        _ => {
            if alpha_type == AlphaType::Opaque {
                "images/color_wheel.jpg"
            } else {
                "images/color_wheel.png"
            }
        }
    };
    let Some(data) = get_resource_as_data(resource) else {
        return;
    };
    let Ok(mut codec) = make_codec_from_stream(Box::new(MemoryStream::from_data(Some(
        Data::new_from_vec(data),
    )))) else {
        return;
    };
    let dst_info = codec
        .info()
        .with_color_type(color_type)
        .with_alpha_type(alpha_type)
        .with_color_space(color_space);
    bitmap.alloc_pixels_info(&dst_info, None);
    if let Some(mut pm) = bitmap.peek_pixels_mut() {
        let row_bytes = pm.row_bytes();
        if let Some(pixels) = pm.writable_addr() {
            // The result is ignored, as in the C++ (`codec->getPixels(...)` is not checked).
            let _ = codec.get_pixels(&dst_info, pixels, row_bytes, None);
        }
    }
}

/// Port of `encode_data(const SkBitmap&, SkEncodedImageFormat)` (encode_srgb.cpp#L45-L66).
fn encode_data(bitmap: &Bitmap, format: EncodedFormat) -> Option<Data> {
    let src = bitmap.peek_pixels()?;
    match format {
        EncodedFormat::Png => png_encoder::encode_pixmap(&src, &png_encoder::Options::default()),
        EncodedFormat::Jpeg => jpeg_encoder::encode_pixmap(&src, &jpeg_encoder::Options::default()),
    }
}

/// Port of `class EncodeSRGBGM` (encode_srgb.cpp#L68-L150).
struct EncodeSrgbGm {
    format: EncodedFormat,
}

impl GM for EncodeSrgbGm {
    fn name(&self) -> String {
        match self.format {
            EncodedFormat::Png => "encode-srgb-png".to_string(),
            EncodedFormat::Jpeg => "encode-srgb-jpg".to_string(),
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(IMAGE_WIDTH * 2, IMAGE_HEIGHT * 15)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        // kGray_8 is not in the list when the NDK encoders are on (not in this port).
        let color_types = [
            ColorType::N32,
            ColorType::RGBAF16,
            ColorType::Gray8,
            ColorType::RGB565,
        ];
        let alpha_types = [AlphaType::Unpremul, AlphaType::Premul, AlphaType::Opaque];
        let color_spaces = [None, Some(ColorSpace::new_srgb())];

        let mut bitmap = Bitmap::new();
        for color_type in color_types {
            for alpha_type in alpha_types {
                canvas.save();
                for color_space in &color_spaces {
                    make(&mut bitmap, color_type, alpha_type, color_space.clone());
                    if let Some(data) = encode_data(&bitmap, self.format)
                        && let Some(image) = deferred_from_encoded_data(Some(data), None)
                    {
                        canvas.draw_image(&image, (0.0, 0.0), None);
                    }
                    canvas.translate((IMAGE_WIDTH_F, 0.0));
                }
                canvas.restore();
                canvas.translate((0.0, IMAGE_HEIGHT_F));
            }
        }
    }
}

// Port of: gm/encode_srgb.cpp#L156 (chrome/m156)
crate::def_gm!(
    #[ignore = "see notes/gm_encode_srgb_cpp_EncodeSRGBGM_kPNG.md"]
    EncodeSRGBGM_kPNG = "EncodeSRGBGM(SkEncodedImageFormat::kPNG)",
    EncodeSrgbGm {
        format: EncodedFormat::Png
    }
);

// Port of: gm/encode_srgb.cpp#L158 (chrome/m156)
crate::def_gm!(
    #[ignore = "see notes/gm_encode_srgb_cpp_EncodeSRGBGM_kJPEG.md"]
    EncodeSRGBGM_kJPEG = "EncodeSRGBGM(SkEncodedImageFormat::kJPEG)",
    EncodeSrgbGm {
        format: EncodedFormat::Jpeg
    }
);
