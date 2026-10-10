// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmapimage.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-scalar conversions of small sizes (exact in f32)

use crate::prelude::*;
use crate::tool_utils::get_resource_as_data;
use skia_rust_codec::codec::Options;
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/bitmapimage.cpp#L19-L70 (chrome/m156)
struct BitmapImageGm;

const K_SIZE: i32 = 512;

impl GM for BitmapImageGm {
    fn name(&self) -> String {
        "bitmap-image-srgb-legacy".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(2 * K_SIZE, 2 * K_SIZE)
    }

    // Port of: gm/bitmapimage.cpp#L27-L60 (onDraw)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        // Create image.
        let path = "images/mandrill_512_q075.jpg";
        let Some(image) = crate::tool_utils::get_resource_as_image(path) else {
            *error_msg = "Couldn't load images/mandrill_512_q075.jpg. \
                          Did you forget to set the resource path?"
                .to_string();
            return DrawResult::Fail;
        };

        // Create matching bitmap.
        let data = get_resource_as_data(path).expect("the resource was just decoded");
        let mut codec = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders())
            .expect("a codec for the resource");
        let codec_image = codec
            .get_image(None::<ImageInfo>, None::<&Options>)
            .expect("the codec image");

        // The GM will be displayed in a 2x2 grid.
        // The top two squares show an sRGB image, then bitmap, drawn to a legacy canvas.
        let linear_info =
            ImageInfo::new_n32((2 * K_SIZE, K_SIZE), AlphaType::Opaque, None::<ColorSpace>);
        let mut legacy_bm_canvas = Bitmap::new();
        legacy_bm_canvas.alloc_pixels_info(&linear_info, None);
        draw_pair(&mut legacy_bm_canvas, &image, &codec_image);
        canvas.draw_image(
            legacy_bm_canvas.as_image().expect("an image"),
            (0.0, 0.0),
            None,
        );
        canvas.translate((0.0, K_SIZE as f32));

        // The bottom two squares show an sRGB image, then bitmap, drawn to a srgb canvas.
        let srgb_info = ImageInfo::new_n32(
            (2 * K_SIZE, K_SIZE),
            AlphaType::Opaque,
            ColorSpace::new_srgb(),
        );
        let mut srgb_bm_canvas = Bitmap::new();
        srgb_bm_canvas.alloc_pixels_info(&srgb_info, None);
        draw_pair(&mut srgb_bm_canvas, &image, &codec_image);
        canvas.draw_image(
            srgb_bm_canvas.as_image().expect("an image"),
            (0.0, 0.0),
            None,
        );
        DrawResult::Ok
    }
}

// The two draws shared by both halves of the GM: `legacyCanvas` / `srgbCanvas`.
fn draw_pair(bitmap: &mut Bitmap, image: &Image, codec_image: &Image) {
    let canvas = skia_rust_core::canvas::Canvas::from_bitmap(bitmap, None).expect("a canvas");
    canvas.draw_image(image, (0.0, 0.0), None);
    canvas.translate((K_SIZE as f32, 0.0));
    canvas.draw_image(codec_image, (0.0, 0.0), None);
}

crate::def_gm!(BitmapImageGM, BitmapImageGm);
