// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/readpixels.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-scalar conversions of small sizes (exact in f32)
#![allow(clippy::cast_sign_loss)] // mirrors the C++ size_t row byte counts of small images
//
// Not ported: `ReadPixelsCodecGM` (its canvas needs a colour space, which the GM harness does not
// give it) and `ReadPixelsPictureGM` (SkImages::DeferredFromPicture is not on main yet).

use crate::GM;
use crate::tool_utils::get_resource_as_data;
use skia_rust_codec::codec::Options;
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color_space::{ColorSpace, ColorSpacePrimaries, ColorSpaceTransferFn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;

const K_WIDTH: i32 = 64;
const K_HEIGHT: i32 = 64;

// Port of: gm/readpixels.cpp#L40-L51 (chrome/m156), make_raster_image
fn make_raster_image(color_type: ColorType) -> Option<Image> {
    let data = get_resource_as_data("images/google_chrome.ico")?;
    let mut codec = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()).ok()?;
    let info = codec
        .info()
        .with_wh(K_WIDTH, K_HEIGHT)
        .with_color_type(color_type)
        .with_alpha_type(AlphaType::Premul);
    codec.get_image(info, None::<&Options>).ok()
}

// Port of: gm/readpixels.cpp#L53-L58 (chrome/m156), make_codec_image
// (the image is only used by `ReadPixelsCodecGM`, which is not ported).
// Port of: gm/readpixels.cpp#L70-L73 (chrome/m156), make_picture_image is not ported.

// Port of: gm/readpixels.cpp#L75-L80 (chrome/m156), make_parametric_transfer_fn
fn make_parametric_transfer_fn(primaries: &ColorSpacePrimaries) -> ColorSpace {
    let to_xyz_d50 = primaries
        .to_xyzd50()
        .expect("primaries with a non-singular matrix");
    let tf = ColorSpaceTransferFn {
        g: 1.8,
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
    };
    ColorSpace::new_rgb(&tf, &to_xyz_d50).expect("a parametric colour space")
}

// Port of: gm/readpixels.cpp#L82-L93 (chrome/m156), make_wide_gamut (ProPhoto)
fn make_wide_gamut() -> ColorSpace {
    let primaries = ColorSpacePrimaries {
        rx: 0.7347,
        ry: 0.2653,
        gx: 0.1596,
        gy: 0.8404,
        bx: 0.0366,
        by: 0.0001,
        wx: 0.34567,
        wy: 0.35850,
    };
    make_parametric_transfer_fn(&primaries)
}

// Port of: gm/readpixels.cpp#L95-L106 (chrome/m156), make_small_gamut
fn make_small_gamut() -> ColorSpace {
    let primaries = ColorSpacePrimaries {
        rx: 0.50,
        ry: 0.33,
        gx: 0.30,
        gy: 0.50,
        bx: 0.25,
        by: 0.16,
        wx: 0.3127,
        wy: 0.3290,
    };
    make_parametric_transfer_fn(&primaries)
}

// Port of: gm/readpixels.cpp#L108-L125 (chrome/m156), draw_image (the GPU context is null on CPU)
fn draw_image(
    canvas: &Canvas,
    image: &Image,
    dst_color_type: ColorType,
    dst_alpha_type: AlphaType,
    dst_color_space: Option<ColorSpace>,
) {
    let row_bytes = image.width() as usize * dst_color_type.bytes_per_pixel();
    let mut data = vec![0u8; row_bytes * image.height() as usize];
    let dst_info = ImageInfo::new(
        (image.width(), image.height()),
        dst_color_type,
        dst_alpha_type,
        dst_color_space,
    );
    if !image.read_pixels(&dst_info, &mut data, row_bytes, (0, 0)) {
        data.fill(0);
    }

    // Now that we have called readPixels(), dump the raw pixels into an srgb image.
    let srgb = ColorSpace::new_srgb();
    if let Some(raw) = images::raster_from_data(
        &dst_info.with_color_space(srgb),
        Data::new_from_vec(data),
        row_bytes,
    ) {
        canvas.draw_image(&raw, (0.0, 0.0), None);
    }
}

// Port of: gm/readpixels.cpp#L132-L187 (chrome/m156), class ReadPixelsGM
struct ReadPixelsGm;

impl GM for ReadPixelsGm {
    fn name(&self) -> String {
        "readpixels".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(6 * K_WIDTH, 9 * K_HEIGHT)
    }

    // Port of: gm/readpixels.cpp#L143-L186 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let alpha_types = [AlphaType::Unpremul, AlphaType::Premul];
        let color_types = [ColorType::RGBA8888, ColorType::BGRA8888, ColorType::RGBAF16];
        let color_spaces = [
            make_wide_gamut(),
            ColorSpace::new_srgb(),
            make_small_gamut(),
        ];

        for dst_color_space in &color_spaces {
            for src_color_type in color_types {
                canvas.save();
                let Some(image) = make_raster_image(src_color_type) else {
                    // The C++ `continue`s here without restoring the save.
                    continue;
                };
                for dst_color_type in color_types {
                    for dst_alpha_type in alpha_types {
                        draw_image(
                            canvas,
                            &image,
                            dst_color_type,
                            dst_alpha_type,
                            Some(dst_color_space.clone()),
                        );
                        canvas.translate((K_WIDTH as f32, 0.0));
                    }
                }
                canvas.restore();
                canvas.translate((0.0, K_HEIGHT as f32));
            }
        }
    }
}

// Port of: gm/readpixels.cpp#L189 (chrome/m156), DEF_GM( return new ReadPixelsGM; )
crate::def_gm!(
    #[ignore = "see notes/gm_readpixels_cpp_ReadPixelsGM.md"]
    ReadPixelsGM = "ReadPixelsGM",
    ReadPixelsGm
);
