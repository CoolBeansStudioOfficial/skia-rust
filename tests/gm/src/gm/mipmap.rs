// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/mipmap.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_raster::surfaces;

// Port of: gm/mipmap.cpp#L20-L35 (chrome/m156)
fn make_image() -> Option<Image> {
    let info = ImageInfo::new_n32_premul((319, 52), None);
    let mut surface = surfaces::raster(&info, None, None)?;
    {
        let canvas = surface.canvas();
        canvas.draw_color(Color::new(0xFFF8_F8F8), None);

        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        paint.set_style(Style::Stroke);
        for _ in 0..20 {
            canvas.draw_circle((-4.0, 25.0), 20.0, &paint);
            canvas.translate((25.0, 0.0));
        }
    }
    surface.image_snapshot()
}

// Port of: gm/mipmap.cpp#L37-L60 (chrome/m156)
crate::def_simple_gm!(mipmap, canvas, 400, 200, {
    let img = make_image().expect("an image"); //SkImage::NewFromEncoded(data));

    let dst = Rect::from_wh(177.0, 15.0);

    // (The label string is only formatted for a commented-out `drawString`.)

    let samplings = [
        SamplingOptions::from(FilterMode::Nearest),
        SamplingOptions::from(FilterMode::Linear),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
        SamplingOptions::from(CubicResampler::mitchell()),
    ];

    canvas.translate((20.0, 20.0));
    for sampling in samplings {
        canvas.draw_image_rect_with_sampling_options(&img, None, dst, sampling, &Paint::default());
        canvas.translate((0.0, 20.0));
    }
    canvas.draw_image(&img, (20.0, 20.0), None);
});

///////////////////////////////////////////////////////////////////////////////////////////////////

// create a circle image computed raw, so we can wrap it as a linear or srgb image
// Port of: gm/mipmap.cpp#L64-L78 (chrome/m156)
fn make(cs: Option<ColorSpace>) -> Option<Image> {
    const N: i32 = 100;
    let info = ImageInfo::new((N, N), ColorType::N32, AlphaType::Premul, cs);
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&info, None);

    for y in 0..N {
        for x in 0..N {
            bm.set_addr32(
                x,
                y,
                if (x ^ y) & 1 != 0 {
                    0xFFFF_FFFF
                } else {
                    0xFF00_0000
                },
            );
        }
    }
    bm.set_immutable();
    bm.as_image()
}

// Draws `img` at the sizes of its mip levels (`show_mips` and `show_mips_only`).
// Port of: gm/mipmap.cpp#L80-L92 and gm/mipmap.cpp#L127-L139 (chrome/m156)
fn show_mips_from(canvas: &Canvas, img: &Image, mut dst: IRect) {
    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);

    // Want to ensure we never draw fractional pixels, so we use an IRect
    while dst.width() > 5 {
        canvas.draw_image_rect_with_sampling_options(
            img,
            None,
            Rect::from_irect(dst),
            sampling,
            &Paint::default(),
        );
        dst.offset((dst.width() + 10, 0));
        dst.right = dst.left + dst.width() / 2;
        dst.bottom = dst.top + dst.height() / 2;
    }
}

// Port of: gm/mipmap.cpp#L80-L92 (chrome/m156)
fn show_mips(canvas: &Canvas, img: &Image) {
    show_mips_from(canvas, img, IRect::from_wh(img.width(), img.height()));
}

/*
 *  Ensure that in L32 drawing mode, both images/mips look the same as each other, and
 *  their mips are darker than the original (since the mips should ignore the gamma in L32).
 *
 *  Ensure that in S32 drawing mode, all images/mips look the same, and look correct (i.e.
 *  the mip levels match the original in brightness).
 */
// Port of: gm/mipmap.cpp#L101-L109 (chrome/m156)
crate::def_simple_gm!(mipmap_srgb, canvas, 260, 230, {
    let limg = make(None).expect("an image");
    let simg = make(Some(ColorSpace::new_srgb())).expect("an image");

    canvas.translate((10.0, 10.0));
    show_mips(canvas, &limg);
    #[allow(clippy::cast_precision_loss)] // limg->height() + 10.0f
    canvas.translate((0.0, limg.height() as f32 + 10.0));
    show_mips(canvas, &simg);
});

///////////////////////////////////////////////////////////////////////////////////////////////////

// create a gradient image computed raw, so we can wrap it as a linear or srgb image
// Port of: gm/mipmap.cpp#L113-L125 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // (x + y) / (2.0f * (N - 1))
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // static_cast<uint8_t>
fn make_g8_gradient(cs: Option<ColorSpace>) -> Option<Image> {
    const N: i32 = 100;
    let info = ImageInfo::new((N, N), ColorType::Gray8, AlphaType::Opaque, cs);
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&info, None);

    for y in 0..N {
        for x in 0..N {
            bm.set_addr8(
                x,
                y,
                (255.0_f32 * ((x + y) as f32 / (2.0_f32 * (N - 1) as f32))) as u8,
            );
        }
    }
    bm.set_immutable();
    bm.as_image()
}

// Port of: gm/mipmap.cpp#L127-L139 (chrome/m156)
fn show_mips_only(canvas: &Canvas, img: &Image) {
    show_mips_from(
        canvas,
        img,
        IRect::from_wh(img.width() / 2, img.height() / 2),
    );
}

/*
 *  Ensure that in L32 drawing mode, both images/mips look the same as each other, and
 *  their mips are darker than the original (since the mips should ignore the gamma in L32).
 *
 *  Ensure that in S32 drawing mode, all images/mips look the same, and look correct (i.e.
 *  the mip levels match the original in brightness).
 */
// Port of: gm/mipmap.cpp#L150-L158 (chrome/m156)
crate::def_simple_gm!(mipmap_gray8_srgb, canvas, 260, 230, {
    let limg = make_g8_gradient(None).expect("an image");
    let simg = make_g8_gradient(Some(ColorSpace::new_srgb())).expect("an image");

    canvas.translate((10.0, 10.0));
    show_mips_only(canvas, &limg);
    #[allow(clippy::cast_precision_loss)] // limg->height() + 10.0f
    canvas.translate((0.0, limg.height() as f32 + 10.0));
    show_mips_only(canvas, &simg);
});
