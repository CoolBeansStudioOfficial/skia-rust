// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefilterscropexpand.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]
// GM ports mirror the C++ GM body, which is long by nature.
#![allow(clippy::too_many_lines)]
use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::{Color4f, ColorChannel};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::{
    blur, color_filter, dilate, displacement_map, drop_shadow, erode, image_sampled, offset,
    point_lit_diffuse,
};

// Port of: gm/imagefilterscropexpand.cpp#L44-L58 (chrome/m156), make_checkerboard
fn make_checkerboard() -> Option<Image> {
    let info = ImageInfo::new((64, 64), ColorType::N32, AlphaType::Premul, None);
    let mut surface = skia_rust_raster::surfaces::raster(&info, None, None)?;
    {
        let canvas = surface.canvas();
        canvas.clear(Color::from(0xFFFF_0000));
        let mut dark_paint = Paint::default();
        dark_paint.set_color(Color::from(0xFF40_4040));
        let mut light_paint = Paint::default();
        light_paint.set_color(Color::from(0xFFA0_A0A0));
        for y in (8..48).step_by(16) {
            for x in (8..48).step_by(16) {
                canvas.save();
                canvas.translate((int_to_scalar(x), int_to_scalar(y)));
                canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 8.0, 8.0), &dark_paint);
                canvas.draw_rect(Rect::from_xywh(8.0, 0.0, 8.0, 8.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(0.0, 8.0, 8.0, 8.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(8.0, 8.0, 8.0, 8.0), &dark_paint);
                canvas.restore();
            }
        }
    }
    surface.image_snapshot()
}

// Port of: gm/imagefilterscropexpand.cpp#L60-L80 (chrome/m156), make_gradient_circle
fn make_gradient_circle(width: i32, height: i32) -> Option<Image> {
    let x = int_to_scalar(width / 2);
    let y = int_to_scalar(height / 2);
    let radius = x.min(y) * 0.8;
    let info = ImageInfo::new((width, height), ColorType::N32, AlphaType::Premul, None);
    let mut surface = skia_rust_raster::surfaces::raster(&info, None, None)?;
    {
        let canvas = surface.canvas();
        canvas.clear(Color::from(0x0000_0000));
        let colors = [Color4f::from(Color::WHITE), Color4f::from(Color::BLACK)];
        let mut paint = Paint::default();
        paint.set_shader(gradient_shaders::radial_gradient(
            ((x, y), radius),
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        canvas.draw_circle((x, y), radius, &paint);
    }
    surface.image_snapshot()
}

// Port of: gm/imagefilterscropexpand.cpp#L82-L101 (chrome/m156), draw
fn draw(canvas: &Canvas, image: &Image, layer_rect: IRect, filter: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(filter);

    // We don't pass 'layerRect' in as the saveLayer bounds hint because that is not the
    // pre-filtering local bounds. Every 'filter' is constructed with a crop rect equal to
    // 'layerRect', so do an unclipped saveLayer to let the crop rect restrict the output.
    canvas.save_layer(&SaveLayerRec::default().paint(&paint));
    canvas.draw_image(image, (0.0, 0.0), None);
    canvas.restore();

    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(Color::from(0xFFFF_0000));
    stroke_paint.set_style(Style::Stroke);
    canvas.draw_rect(Rect::from(layer_rect), &stroke_paint);

    canvas.translate((int_to_scalar(80), 0.0));
}

// Port of: gm/imagefilterscropexpand.cpp#L23-L99 (chrome/m156), DEF_SIMPLE_GM(imagefilterscropexpand)
crate::def_simple_gm!(imagefilterscropexpand, canvas, 730, 650, {
    let crop_rect = IRect::from_xywh(10, 10, 44, 44);

    let gradient_circle = make_gradient_circle(64, 64);
    let checkerboard = make_checkerboard().expect("a checkerboard image");

    let gradient_circle_source =
        image_sampled(gradient_circle, SamplingOptions::from(FilterMode::Linear));
    let noop_cropped = offset((0.0, 0.0), None, Some(Rect::from(crop_rect)));
    // This color matrix saturates the green component but only partly increases the opacity.
    // For the opaque checkerboard, the opacity boost doesn't matter but it does impact the
    // area outside the checkerboard.
    #[rustfmt::skip]
    let matrix: [f32; 20] = [
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0, 1.0,
        0.0, 0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0, 32.0_f32 / 255.0,
    ];
    let cf_alpha_trans = color_filters::matrix_row_major(&matrix, Clamp::Yes);

    let point_location = Point3::new(0.0, 0.0, int_to_scalar(10));
    let kd: f32 = int_to_scalar(2);
    let surface_scale: f32 = int_to_scalar(1);

    canvas.translate((12.0, 12.0));
    for outset in (-15..=20).step_by(5) {
        canvas.save();
        let mut big_rect = crop_rect;
        big_rect.outset((outset, outset));
        let big = Rect::from(big_rect);

        draw(
            canvas,
            &checkerboard,
            big_rect,
            color_filter(cf_alpha_trans.clone(), noop_cropped.clone(), Some(big)),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            blur(0.3, 0.3, TileMode::Decal, noop_cropped.clone(), Some(big)),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            blur(8.0, 8.0, TileMode::Decal, noop_cropped.clone(), Some(big)),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            dilate((2.0, 2.0), noop_cropped.clone(), Some(big)),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            erode((2.0, 2.0), noop_cropped.clone(), Some(big)),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            drop_shadow(
                (10.0, 10.0),
                (3.0, 3.0),
                Color4f::from(Color::BLUE),
                None,
                noop_cropped.clone(),
                Some(big),
            ),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            displacement_map(
                (ColorChannel::R, ColorChannel::R),
                12.0,
                gradient_circle_source.clone(),
                noop_cropped.clone(),
                Some(big),
            ),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            offset(
                (int_to_scalar(-8), int_to_scalar(16)),
                noop_cropped.clone(),
                Some(big),
            ),
        );

        draw(
            canvas,
            &checkerboard,
            big_rect,
            point_lit_diffuse(
                point_location,
                Color::WHITE,
                surface_scale,
                kd,
                noop_cropped.clone(),
                Some(big),
            ),
        );

        canvas.restore();
        canvas.translate((0.0, int_to_scalar(80)));
    }
});
