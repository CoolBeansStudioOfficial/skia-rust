// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/makecolorspace.cpp (chrome/m156)

use skia_rust_codec::codec::Options;
use skia_rust_codec::{Codec, decoders};
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::MemoryStream;

use crate::prelude::*;
use crate::tool_utils::{get_resource_as_data, get_resource_as_image};

// Port of: gm/makecolorspace.cpp#L35-L41 (chrome/m156), make_raster_image
fn make_raster_image(path: &str) -> Option<Image> {
    let data = get_resource_as_data(path)?;
    let mut codec = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()).ok()?;
    codec.get_image(None::<ImageInfo>, None::<&Options>).ok()
}

// Port of: gm/makecolorspace.cpp#L42-L64 (chrome/m156), make_color_space
fn make_color_space(orig: Option<&Image>, color_space: &ColorSpace) -> Option<Image> {
    let orig = orig?;
    let xform = orig.make_color_space(color_space.clone(), RequiredProperties::default())?;
    // Assign an sRGB color space on the xformed image, so we can see the effects of the xform
    // when we draw.
    let srgb = if color_space.gamma_is_linear() {
        ColorSpace::new_srgb_linear()
    } else {
        ColorSpace::new_srgb()
    };
    xform.reinterpret_color_space(srgb)
}

// `canvas->drawImage(image, x, y)` for an image that may be null (`sk_sp` may be empty).
fn draw_opt(canvas: &Canvas, image: Option<&Image>, x: f32, y: f32) {
    if let Some(image) = image {
        canvas.draw_image(image, (x, y), None);
    }
}

// Port of: gm/makecolorspace.cpp#L66-L97 (chrome/m156), DEF_SIMPLE_GM_CAN_FAIL(makecolorspace, ...)
crate::def_simple_gm_can_fail!(makecolorspace, canvas, error_msg, 128 * 3, 128 * 4, {
    let wide_gamut = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::ADOBE_RGB)
        .expect("a valid color space");
    let wide_gamut_linear = wide_gamut.make_linear_gamma();

    // Lazy images
    let opaque_image = get_resource_as_image("images/mandrill_128.png");
    let premul_image = get_resource_as_image("images/color_wheel.png");
    let (Some(opaque_image), Some(premul_image)) = (opaque_image, premul_image) else {
        error_msg.clear();
        error_msg.push_str("Failed to load images. Did you forget to set the resourcePath?");
        return DrawResult::Fail;
    };
    canvas.draw_image(&opaque_image, (0.0, 0.0), None);
    draw_opt(
        canvas,
        make_color_space(Some(&opaque_image), &wide_gamut).as_ref(),
        128.0,
        0.0,
    );
    draw_opt(
        canvas,
        make_color_space(Some(&opaque_image), &wide_gamut_linear).as_ref(),
        256.0,
        0.0,
    );
    canvas.draw_image(&premul_image, (0.0, 128.0), None);
    draw_opt(
        canvas,
        make_color_space(Some(&premul_image), &wide_gamut).as_ref(),
        128.0,
        128.0,
    );
    draw_opt(
        canvas,
        make_color_space(Some(&premul_image), &wide_gamut_linear).as_ref(),
        256.0,
        128.0,
    );
    canvas.translate((0.0, 256.0));

    // Raster images
    let opaque_image = make_raster_image("images/mandrill_128.png");
    let premul_image = make_raster_image("images/color_wheel.png");
    draw_opt(canvas, opaque_image.as_ref(), 0.0, 0.0);
    draw_opt(
        canvas,
        make_color_space(opaque_image.as_ref(), &wide_gamut).as_ref(),
        128.0,
        0.0,
    );
    draw_opt(
        canvas,
        make_color_space(opaque_image.as_ref(), &wide_gamut_linear).as_ref(),
        256.0,
        0.0,
    );
    draw_opt(canvas, premul_image.as_ref(), 0.0, 128.0);
    draw_opt(
        canvas,
        make_color_space(premul_image.as_ref(), &wide_gamut).as_ref(),
        128.0,
        128.0,
    );
    draw_opt(
        canvas,
        make_color_space(premul_image.as_ref(), &wide_gamut_linear).as_ref(),
        256.0,
        128.0,
    );
    DrawResult::Ok
});

// Port of: gm/makecolorspace.cpp#L99-L164 (chrome/m156), DEF_SIMPLE_GM_BG(makecolortypeandspace, ...)
crate::def_simple_gm_bg!(makecolortypeandspace, canvas, 128 * 3, 128 * 4, Color::WHITE, {
    let mut images: [Option<Image>; 2] = [
        get_resource_as_image("images/mandrill_128.png"),
        get_resource_as_image("images/color_wheel.png"),
    ];
    let rec2020 = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::REC2020)
        .expect("a valid color space");

    // Use the lazy images on the first iteration, and concrete (raster) images on the second
    for lazy in [true, false] {
        for slot in &mut images {
            // `continue` in the C++ skips the translate below as well.
            let Some(image) = slot.clone() else {
                continue;
            };

            // Unmodified
            canvas.draw_image(&image, (0.0, 0.0), None);

            // 565 in a wide color space (should be visibly quantized).
            let image565 = image.make_color_type_and_color_space(
                ColorType::RGB565,
                rec2020.clone(),
                RequiredProperties::default(),
            );
            if let Some(image565) = image565 {
                if !lazy || image565.is_texture_backed() || image565.to_raster_image().is_some() {
                    canvas.draw_image(&image565, (128.0, 0.0), None);
                }
            }

            // Grayscale in the original color space.
            let image_gray = image.make_color_type_and_color_space(
                ColorType::Gray8,
                image.color_space(),
                RequiredProperties::default(),
            );
            if let Some(image_gray) = image_gray {
                if !lazy || image_gray.is_texture_backed() || image_gray.to_raster_image().is_some()
                {
                    canvas.draw_image(&image_gray, (256.0, 0.0), None);
                }
            }

            // No GPU context: the raster copy.
            *slot = image.to_raster_image();
            canvas.translate((0.0, 128.0));
        }
    }
});

// Port of: gm/makecolorspace.cpp#L166-L231 (chrome/m156), DEF_SIMPLE_GM_CAN_FAIL(reinterpretcolorspace, ...)
crate::def_simple_gm_can_fail!(reinterpretcolorspace, canvas, error_msg, 128 * 3, 128 * 3, {
    // We draw a 3x3 grid. The three rows are lazy (encoded), raster, and GPU (or another copy of
    // raster so all configs look similar). In each row, we draw three variants.
    let srgb = ColorSpace::new_srgb();
    let spin = srgb.make_color_spin();
    let Some(image) = get_resource_as_image("images/color_wheel.png") else {
        error_msg.clear();
        error_msg.push_str("Failed to load image. Did you forget to set the resourcePath?");
        return DrawResult::Fail;
    };

    // Lazy images
    canvas.draw_image(&image, (0.0, 0.0), None);
    draw_opt(
        canvas,
        image.reinterpret_color_space(spin.clone()).as_ref(),
        128.0,
        0.0,
    );
    let spun = image.make_color_space(spin.clone(), RequiredProperties::default());
    draw_opt(
        canvas,
        spun.and_then(|spun| spun.reinterpret_color_space(srgb.clone()))
            .as_ref(),
        256.0,
        0.0,
    );
    canvas.translate((0.0, 128.0));

    // Raster images
    let image = image.to_raster_image();
    draw_opt(canvas, image.as_ref(), 0.0, 0.0);
    draw_opt(
        canvas,
        image
            .as_ref()
            .and_then(|image| image.reinterpret_color_space(spin.clone()))
            .as_ref(),
        128.0,
        0.0,
    );
    let spun = image
        .as_ref()
        .and_then(|image| image.make_color_space(spin.clone(), RequiredProperties::default()));
    draw_opt(
        canvas,
        spun.and_then(|spun| spun.reinterpret_color_space(srgb.clone()))
            .as_ref(),
        256.0,
        0.0,
    );
    canvas.translate((0.0, 128.0));

    // GPU images: there is no GPU context, so the GPU row is another copy of the raster image.
    draw_opt(canvas, image.as_ref(), 0.0, 0.0);
    draw_opt(
        canvas,
        image
            .as_ref()
            .and_then(|image| image.reinterpret_color_space(spin.clone()))
            .as_ref(),
        128.0,
        0.0,
    );
    let spun = image
        .as_ref()
        .and_then(|image| image.make_color_space(spin.clone(), RequiredProperties::default()));
    draw_opt(
        canvas,
        spun.and_then(|spun| spun.reinterpret_color_space(srgb.clone()))
            .as_ref(),
        256.0,
        0.0,
    );
    DrawResult::Ok
});
