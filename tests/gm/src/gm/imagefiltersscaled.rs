// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersscaled.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::{create_checkerboard_image, int_to_scalar};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Color4f, ColorChannel};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point3::Point3;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar_invert;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::shader_filter::Dither;
use skia_rust_effects::image_filters::{
    blur, displacement_map, dilate, drop_shadow, erode, image_sampled, matrix_transform, offset,
    point_lit_diffuse, shader as shader_filter, spot_lit_diffuse,
};
use skia_rust_effects::perlin_noise_shader::shaders::fractal_noise;

const RESIZE_FACTOR: f32 = 4.0;

// Port of: gm/imagefiltersscaled.cpp#L14-L28 (chrome/m156), make_gradient_circle
fn make_gradient_circle(width: i32, height: i32) -> Option<Image> {
    let x = int_to_scalar(width / 2);
    let y = int_to_scalar(height / 2);
    let radius = x.min(y) * 4.0 / 5.0;
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

// Port of: gm/imagefiltersscaled.cpp#L63-L164 (chrome/m156), ImageFiltersScaledGM
struct ImageFiltersScaledGm {
    checkerboard: Option<Image>,
    gradient_circle: Option<Image>,
}

impl GM for ImageFiltersScaledGm {
    fn name(&self) -> String {
        "imagefiltersscaled".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1428, 500)
    }

    fn bg_color(&self) -> Color {
        Color::from(0x0000_0000)
    }

    // Port of: gm/imagefiltersscaled.cpp#L72-L75 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.checkerboard = Some(create_checkerboard_image(
            64,
            64,
            Color::from(0xFFA0_A0A0),
            Color::from(0xFF40_4040),
            8,
        ));
        self.gradient_circle = make_gradient_circle(64, 64);
    }

    // Port of: gm/imagefiltersscaled.cpp#L77-L163 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::BLACK);
        let (Some(gradient_circle), Some(checkerboard)) =
            (self.gradient_circle.clone(), self.checkerboard.clone())
        else {
            return;
        };
        let gradient = image_sampled(Some(gradient_circle), SamplingOptions::from(FilterMode::Linear));
        let checkerboard_filter = image_sampled(
            Some(checkerboard),
            SamplingOptions::from(FilterMode::Linear),
        );

        let point_location = Point3::new(0.0, 0.0, int_to_scalar(10));
        let spot_location = Point3::new(int_to_scalar(-10), int_to_scalar(-10), int_to_scalar(20));
        let spot_target = Point3::new(int_to_scalar(40), int_to_scalar(40), 0.0);
        let spot_exponent: f32 = 1.0;
        let cutoff_angle = int_to_scalar(15);
        let kd = int_to_scalar(2);
        let surface_scale = int_to_scalar(1);
        let white = Color::WHITE;
        let mut resize_matrix = Matrix::new_identity();
        resize_matrix.set_scale((RESIZE_FACTOR, RESIZE_FACTOR), None);

        let filters: [Option<ImageFilter>; 10] = [
            blur(4.0, 4.0, TileMode::Decal, None, None),
            drop_shadow(
                (5.0, 10.0),
                (3.0, 3.0),
                Color4f::from(Color::YELLOW),
                None,
                None,
                None,
            ),
            displacement_map(
                (ColorChannel::R, ColorChannel::R),
                12.0,
                gradient,
                checkerboard_filter.clone(),
                None,
            ),
            dilate((1.0, 1.0), checkerboard_filter.clone(), None),
            erode((1.0, 1.0), checkerboard_filter, None),
            offset((32.0, 0.0), None, None),
            matrix_transform(&resize_matrix, SamplingOptions::default(), None),
            shader_filter(
                fractal_noise((0.1, 0.05), 1, 0.0, None),
                Dither::No,
                None,
            ),
            point_lit_diffuse(point_location, white, surface_scale, kd, None, None),
            spot_lit_diffuse(
                spot_location,
                spot_target,
                spot_exponent,
                cutoff_angle,
                white,
                surface_scale,
                kd,
                None,
                None,
            ),
        ];

        let scales: [(f32, f32); 5] = [
            (scalar_invert(2.0), scalar_invert(2.0)),
            (int_to_scalar(1), int_to_scalar(1)),
            (int_to_scalar(1), int_to_scalar(2)),
            (int_to_scalar(2), int_to_scalar(1)),
            (int_to_scalar(2), int_to_scalar(2)),
        ];

        let r = Rect::from_wh(int_to_scalar(64), int_to_scalar(64));
        let margin = int_to_scalar(16);
        for &(sx, sy) in &scales {
            canvas.save();
            for (i, filter) in filters.iter().enumerate() {
                let mut paint = Paint::default();
                paint.set_color(Color::BLUE);
                paint.set_image_filter(filter.clone());
                paint.set_anti_alias(true);
                canvas.save();
                canvas.scale((sx, sy));
                canvas.clip_rect(r, None, None);
                if i == 5 {
                    canvas.translate((int_to_scalar(-32), 0.0));
                } else if i == 6 {
                    canvas.scale((scalar_invert(RESIZE_FACTOR), scalar_invert(RESIZE_FACTOR)));
                }
                canvas.draw_circle(
                    (r.center_x(), r.center_y()),
                    r.width() * 2.0 / 5.0,
                    &paint,
                );
                canvas.restore();
                canvas.translate((r.width() * sx + margin, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, r.height() * sy + margin));
        }
    }
}

// Port of: gm/imagefiltersscaled.cpp#L164 (chrome/m156), DEF_GM(return new ImageFiltersScaledGM;)
crate::def_gm!(
    ImageFiltersScaledGM,
    ImageFiltersScaledGm {
        checkerboard: None,
        gradient_circle: None
    }
);
