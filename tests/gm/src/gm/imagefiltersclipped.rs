// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersclipped.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss, clippy::too_many_lines)]

use crate::prelude::*;
use crate::tool_utils::{create_checkerboard_image, int_to_scalar};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color::ColorChannel;
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
use skia_rust_effects::image_filters::{
    blur, dilate, displacement_map, drop_shadow, erode, image_sampled, matrix_transform, offset,
    point_lit_diffuse, shader as shader_filter,
};
use skia_rust_effects::image_filters::shader_filter::Dither;
use skia_rust_effects::perlin_noise_shader::shaders::fractal_noise;

const RESIZE_FACTOR_X: f32 = 2.0;
const RESIZE_FACTOR_Y: f32 = 5.0;

// Port of: gm/imagefiltersclipped.cpp#L13-L27 (chrome/m156), make_gradient_circle
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

// Port of: gm/imagefiltersclipped.cpp#L29-L44 (chrome/m156), draw_clipped_filter
fn draw_clipped_filter(
    canvas: &Canvas,
    filter: Option<ImageFilter>,
    i: usize,
    prim_bounds: Rect,
    clip_bounds: Rect,
) {
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_image_filter(filter);
    paint.set_anti_alias(true);
    canvas.save();
    canvas.clip_rect(clip_bounds, None, None);
    if i == 5 {
        canvas.translate((int_to_scalar(16), int_to_scalar(-32)));
    } else if i == 6 {
        canvas.scale((scalar_invert(RESIZE_FACTOR_X), scalar_invert(RESIZE_FACTOR_Y)));
    }
    canvas.draw_circle(
        (prim_bounds.center_x(), prim_bounds.center_y()),
        prim_bounds.width() * 2.0 / 5.0,
        &paint,
    );
    canvas.restore();
}

// Port of: gm/imagefiltersclipped.cpp#L46-L118 (chrome/m156), ImageFiltersClippedGM
struct ImageFiltersClippedGm {
    checkerboard: Option<Image>,
    gradient_circle: Option<Image>,
}

impl GM for ImageFiltersClippedGm {
    fn name(&self) -> String {
        "imagefiltersclipped".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(860, 500)
    }

    fn bg_color(&self) -> Color {
        Color::from(0x0000_0000)
    }

    // Port of: gm/imagefiltersclipped.cpp#L55-L59 (chrome/m156), onOnceBeforeDraw
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

    // Port of: gm/imagefiltersclipped.cpp#L60-L117 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::BLACK);
        let (Some(gradient_circle), Some(checkerboard)) =
            (self.gradient_circle.clone(), self.checkerboard.clone())
        else {
            return;
        };
        let gradient = image_sampled(Some(gradient_circle), SamplingOptions::from(FilterMode::Linear));
        let checkerboard_filter = image_sampled(
            Some(checkerboard.clone()),
            SamplingOptions::from(FilterMode::Linear),
        );
        let mut resize_matrix = Matrix::new_identity();
        resize_matrix.set_scale((RESIZE_FACTOR_X, RESIZE_FACTOR_Y), None);
        let point_location = Point3::new(32.0, 32.0, int_to_scalar(10));
        let r = Rect::new(0.0, 0.0, 64.0, 64.0);

        let filters: [Option<ImageFilter>; 8] = [
            blur(12.0, 12.0, TileMode::Decal, None, None),
            drop_shadow(
                (10.0, 10.0),
                (3.0, 3.0),
                Color4f::from(Color::GREEN),
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
            dilate((2.0, 2.0), checkerboard_filter.clone(), None),
            erode((2.0, 2.0), checkerboard_filter.clone(), None),
            offset((-16.0, 32.0), None, None),
            matrix_transform(&resize_matrix, SamplingOptions::default(), None),
            // Crop output of lighting to the checkerboard
            point_lit_diffuse(
                point_location,
                Color::WHITE,
                1.0,
                2.0,
                checkerboard_filter,
                Some(r),
            ),
        ];

        let margin = int_to_scalar(16);
        let mut outset = r;
        outset.outset((margin, margin));
        canvas.save();
        for x_offset in (0..80).step_by(16) {
            canvas.save();
            let bounds = Rect::new(
                int_to_scalar(x_offset),
                outset.top(),
                outset.right(),
                outset.bottom(),
            );
            for (i, filter) in filters.iter().enumerate() {
                draw_clipped_filter(canvas, filter.clone(), i, r, bounds);
                canvas.translate((r.width() + margin, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, r.height() + margin));
        }
        canvas.restore();
        let rect_filter = shader_filter(
            Some(
                fractal_noise((0.1, 0.05), 1, 0.0, None)
                    .expect("a fractal noise shader"),
            ),
            Dither::No,
            None,
        );
        canvas.translate((filters.len() as f32 * (r.width() + margin), 0.0));
        for x_offset in (0..80).step_by(16) {
            let bounds = Rect::new(
                int_to_scalar(x_offset),
                outset.top(),
                outset.right(),
                outset.bottom(),
            );
            draw_clipped_filter(canvas, rect_filter.clone(), 0, r, bounds);
            canvas.translate((0.0, r.height() + margin));
        }
    }
}

// Port of: gm/imagefiltersclipped.cpp#L118 (chrome/m156)
crate::def_gm!(ImageFiltersClippedGM, ImageFiltersClippedGm { checkerboard: None, gradient_circle: None });
