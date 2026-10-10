// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lighting.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: int/float conversions, local constants and long
// bodies are kept as they are there.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::too_many_lines
)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{degrees_to_radians, scalar_cos, scalar_sin};
use skia_rust_effects::image_filters::{
    distant_lit_diffuse, distant_lit_specular, offset, point_lit_diffuse, point_lit_specular,
    spot_lit_diffuse, spot_lit_specular,
};
use skia_rust_tools::font_tool_utils::create_string_bitmap;

const WIDTH: i32 = 660;
const HEIGHT: i32 = 660;

/// The azimuth the GM starts at (`kStartAzimuth`).
const START_AZIMUTH: i32 = 225;

/// `ImageLightingGM`.
// Port of: gm/lighting.cpp#L22-L178 (chrome/m156)
struct ImageLightingGm {
    azimuth: f32,
    bitmap: Option<Bitmap>,
}

/// `ImageLightingGM::drawClippedBitmap`.
// Port of: gm/lighting.cpp#L43-L49 (chrome/m156)
fn draw_clipped_bitmap(canvas: &Canvas, bitmap: &Bitmap, paint: &Paint, x: i32, y: i32) {
    canvas.save();
    canvas.translate((x as f32, y as f32));
    canvas.clip_irect(bitmap.bounds(), None);
    if let Some(image) = bitmap.as_image() {
        canvas.draw_image_with_sampling_options(
            &image,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(paint),
        );
    }
    canvas.restore();
}

impl GM for ImageLightingGm {
    fn name(&self) -> String {
        "lighting".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFF00_0000)
    }

    // Port of: gm/lighting.cpp#L53-L55 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.bitmap = Some(create_string_bitmap(
            100,
            100,
            Color::new(0xFFFF_FFFF),
            20,
            70,
            96,
            "e",
        ));
    }

    // Port of: gm/lighting.cpp#L57-L173 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(bitmap) = self.bitmap.as_ref() else {
            return;
        };
        canvas.clear(Color::new(0xFF10_1010));
        let mut check_paint = Paint::default();
        check_paint.set_color(Color::new(0xFF20_2020));
        for y in (0..HEIGHT).step_by(16) {
            for x in (0..WIDTH).step_by(16) {
                canvas.save();
                canvas.translate((x as f32, y as f32));
                canvas.draw_rect(Rect::from_xywh(8.0, 0.0, 8.0, 8.0), &check_paint);
                canvas.draw_rect(Rect::from_xywh(0.0, 8.0, 8.0, 8.0), &check_paint);
                canvas.restore();
            }
        }

        let azimuth = self.azimuth;
        let sin_azimuth = scalar_sin(degrees_to_radians(azimuth));
        let cos_azimuth = scalar_cos(degrees_to_radians(azimuth));
        let spot_target = Point3::new(40.0, 40.0, 0.0);
        let spot_location = Point3::new(
            spot_target.x + 70.7214 * cos_azimuth,
            spot_target.y + 70.7214 * sin_azimuth,
            spot_target.z + 20.0,
        );
        let spot_exponent1 = 1.0;
        let spot_exponent10 = 10.0;
        let cutoff_angle_small = 15.0;
        let cutoff_angle_none = 180.0;
        let point_location = Point3::new(
            spot_target.x + 50.0 * cos_azimuth,
            spot_target.y + 50.0 * sin_azimuth,
            10.0,
        );
        let elevation_rad = degrees_to_radians(5.0);
        let distant_direction = Point3::new(
            cos_azimuth * scalar_cos(elevation_rad),
            sin_azimuth * scalar_cos(elevation_rad),
            scalar_sin(elevation_rad),
        );
        let kd = 2.0;
        let ks = 1.0;
        let shininess = 8.0;
        let surface_scale = 1.0;
        let surface_scale_small = 0.1;
        let green_yellow = Color::from_argb(255, 173, 255, 47);
        let mut paint = Paint::default();
        let crop_rect = IRect::from_xywh(20, 10, 60, 65);
        let full_size_crop_rect = IRect::from_xywh(0, 0, 100, 100);
        let noop_cropped: Option<ImageFilter> =
            offset((0.0, 0.0), None, Some(Rect::from_irect(crop_rect)));

        let mut y = 0;
        for i in 0..3 {
            // `cr`: the crop rect for this pass; `input`: the noop-cropped offset for the last one.
            let cr: Option<Rect> = match i {
                1 => Some(Rect::from_irect(crop_rect)),
                2 => Some(Rect::from_irect(full_size_crop_rect)),
                _ => None,
            };
            let input: Option<ImageFilter> = if i == 2 { noop_cropped.clone() } else { None };

            // Basic point, distant and spot lights with diffuse lighting
            paint.set_image_filter(point_lit_diffuse(
                point_location,
                Color::WHITE,
                surface_scale,
                kd,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 0, y);
            paint.set_image_filter(distant_lit_diffuse(
                distant_direction,
                Color::WHITE,
                surface_scale,
                kd,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 110, y);
            paint.set_image_filter(spot_lit_diffuse(
                spot_location,
                spot_target,
                spot_exponent1,
                cutoff_angle_small,
                Color::WHITE,
                surface_scale,
                kd,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 220, y);
            // Spot light with no angle cutoff
            paint.set_image_filter(spot_lit_diffuse(
                spot_location,
                spot_target,
                spot_exponent10,
                cutoff_angle_none,
                Color::WHITE,
                surface_scale,
                kd,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 330, y);
            // Spot light with falloff exponent
            paint.set_image_filter(spot_lit_diffuse(
                spot_location,
                spot_target,
                spot_exponent1,
                cutoff_angle_none,
                Color::WHITE,
                surface_scale_small,
                kd,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 440, y);
            // Large constant to show oversaturation
            paint.set_image_filter(distant_lit_diffuse(
                distant_direction,
                green_yellow,
                surface_scale,
                4.0 * kd,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 550, y);
            y += 110;
            // Basic point, distant and spot lights with specular lighting
            paint.set_image_filter(point_lit_specular(
                point_location,
                Color::WHITE,
                surface_scale,
                ks,
                shininess,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 0, y);
            paint.set_image_filter(distant_lit_specular(
                distant_direction,
                Color::WHITE,
                surface_scale,
                ks,
                shininess,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 110, y);
            paint.set_image_filter(spot_lit_specular(
                spot_location,
                spot_target,
                spot_exponent1,
                cutoff_angle_small,
                Color::WHITE,
                surface_scale,
                ks,
                shininess,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 220, y);
            // Spot light with no angle cutoff
            paint.set_image_filter(spot_lit_specular(
                spot_location,
                spot_target,
                spot_exponent10,
                cutoff_angle_none,
                Color::WHITE,
                surface_scale,
                ks,
                shininess,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 330, y);
            // Spot light with falloff exponent
            paint.set_image_filter(spot_lit_specular(
                spot_location,
                spot_target,
                spot_exponent1,
                cutoff_angle_none,
                Color::WHITE,
                surface_scale_small,
                ks,
                shininess,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 440, y);
            // Large constant to show oversaturation
            paint.set_image_filter(distant_lit_specular(
                distant_direction,
                green_yellow,
                surface_scale,
                4.0 * ks,
                shininess,
                input.clone(),
                cr,
            ));
            draw_clipped_bitmap(canvas, bitmap, &paint, 550, y);
            y += 110;
        }
    }
}

// Port of: gm/lighting.cpp#L189 (chrome/m156)
crate::def_gm!(
    ImageLightingGM,
    ImageLightingGm {
        azimuth: START_AZIMUTH as f32,
        bitmap: None,
    }
);
