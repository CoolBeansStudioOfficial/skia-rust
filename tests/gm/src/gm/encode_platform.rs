// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/encode_platform.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::get_resource_as_bitmap;
use skia_rust_codec::encode::{jpeg_encoder, png_encoder, webp_encoder};
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::data::Data;
use skia_rust_core::rect::IRect;

// Port of: gm/encode_platform.cpp#L28-L37 (chrome/m156), the encoded formats of gRecs
#[derive(Clone, Copy)]
enum Format {
    Png,
    Jpeg,
    Webp,
}

// Port of: gm/encode_platform.cpp#L30-L37 (chrome/m156), gRecs
const G_RECS: [(Format, i32); 5] = [
    // We don't support GIF, BMP, or ICO. This applies to both NDK and SkEncoder.
    (Format::Png, 100),
    (Format::Jpeg, 100),
    (Format::Webp, 100), // Lossless
    (Format::Webp, 80),  // Lossy
    (Format::Png, 100),
];

// Port of: gm/encode_platform.cpp#L39-L58 (chrome/m156), encode_data
fn encode_data(format: Format, bitmap: &Bitmap, quality: i32) -> Option<Data> {
    let src = bitmap.peek_pixels()?;
    match format {
        Format::Png => png_encoder::encode_pixmap(&src, &png_encoder::Options::default()),
        Format::Jpeg => {
            let opts = jpeg_encoder::Options {
                quality: quality as u32,
                ..jpeg_encoder::Options::default()
            };
            jpeg_encoder::encode_pixmap(&src, &opts)
        }
        Format::Webp => {
            let opts = webp_encoder::Options {
                quality: quality as f32,
                ..webp_encoder::Options::default()
            };
            webp_encoder::encode_pixmap(&src, &opts)
        }
    }
}

// Port of: gm/encode_platform.cpp#L60-L115 (chrome/m156), EncodePlatformGM
struct EncodePlatformGm;

impl GM for EncodePlatformGm {
    // Port of: gm/encode_platform.cpp#L65-L67 (chrome/m156), getName
    fn name(&self) -> String {
        "encode-platform".to_string()
    }

    // Port of: gm/encode_platform.cpp#L68 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(256 * G_RECS.len() as i32, 256 * 3)
    }

    // Port of: gm/encode_platform.cpp#L69-L100 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let Some(opaque_bm) = get_resource_as_bitmap("images/mandrill_256.png") else {
            "Could not load images/mandrill_256.png.png. Did you forget to set the resourcePath?"
                .clone_into(error_msg);
            return DrawResult::Fail;
        };
        let Some(tmp) = get_resource_as_bitmap("images/yellow_rose.png") else {
            "Could not load images/yellow_rose.png. Did you forget to set the resourcePath?"
                .clone_into(error_msg);
            return DrawResult::Fail;
        };
        let mut premul_bm = Bitmap::new();
        assert!(tmp.extract_subset(&mut premul_bm, IRect::from_wh(256, 256)));
        drop(tmp);

        let mut unpremul_bm = Bitmap::new();
        unpremul_bm.alloc_pixels_flags(&premul_bm.info().with_alpha_type(AlphaType::Unpremul));
        let mut unpremul_pixmap = unpremul_bm
            .peek_pixels_mut()
            .expect("the unpremul bitmap has pixels");
        assert!(premul_bm.read_pixels_to_pixmap(&mut unpremul_pixmap, (0, 0)));

        for (format, quality) in G_RECS {
            let opaque_image =
                deferred_from_encoded_data(encode_data(format, &opaque_bm, quality), None);
            let premul_image =
                deferred_from_encoded_data(encode_data(format, &premul_bm, quality), None);
            let unpremul_image =
                deferred_from_encoded_data(encode_data(format, &unpremul_bm, quality), None);
            if let Some(image) = opaque_image {
                canvas.draw_image(&image, (0.0, 0.0), None);
            }
            if let Some(image) = premul_image {
                canvas.draw_image(&image, (0.0, 256.0), None);
            }
            if let Some(image) = unpremul_image {
                canvas.draw_image(&image, (0.0, 512.0), None);
            }
            canvas.translate((256.0, 0.0));
        }
        DrawResult::Ok
    }
}

// Port of: gm/encode_platform.cpp#L116 (chrome/m156)
crate::def_gm!(EncodePlatformGM, EncodePlatformGm);
