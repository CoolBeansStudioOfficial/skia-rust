// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefilterstransformed.cpp (chrome/m156)
//
// Not ported here: `imagefilter_transformed_image` (it needs `Canvas::concat(SkM44)`, which
// skia-rust does not have) and `ImageFilterMatrixWLocalMatrix` (it needs
// `SkImageFilter::makeWithLocalMatrix`, which skia-rust does not have).

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::{create_checkerboard_image, get_resource_as_image, int_to_scalar};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color4f, ColorChannel};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::{
    blend, blur, compose, dilate, displacement_map, drop_shadow, erode, image_sampled,
    matrix_transform, offset,
};

// Port of: gm/imagefilterstransformed.cpp#L30-L44 (chrome/m156), make_gradient_circle
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

// Port of: gm/imagefilterstransformed.cpp#L46-L115 (chrome/m156), ImageFiltersTransformedGM
struct ImageFiltersTransformedGm {
    checkerboard: Option<Image>,
    gradient_circle: Option<Image>,
}

impl GM for ImageFiltersTransformedGm {
    fn name(&self) -> String {
        "imagefilterstransformed".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(420, 240)
    }

    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/imagefilterstransformed.cpp#L56-L60 (chrome/m156), onOnceBeforeDraw
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

    // Port of: gm/imagefilterstransformed.cpp#L61-L113 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
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

        let filters: [Option<ImageFilter>; 5] = [
            blur(12.0, 0.0, TileMode::Decal, None, None),
            drop_shadow(
                (0.0, 15.0),
                (8.0, 0.0),
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
            erode((2.0, 2.0), checkerboard_filter, None),
        ];
        let margin = int_to_scalar(20);
        let size = int_to_scalar(60);
        for j in 0..3 {
            canvas.save();
            canvas.translate((margin, 0.0));
            for filter in &filters {
                let mut paint = Paint::default();
                paint.set_color(Color::WHITE);
                paint.set_image_filter(filter.clone());
                paint.set_anti_alias(true);
                canvas.save();
                canvas.translate((size * 0.5, size * 0.5));
                canvas.scale((0.8, 0.8));
                if j == 1 {
                    canvas.rotate(45.0, None);
                } else if j == 2 {
                    canvas.skew((0.5, 0.2));
                }
                canvas.translate((-size * 0.5, -size * 0.5));
                canvas.draw_oval(
                    Rect::from_xywh(0.0, size * 0.1, size, size * 0.6),
                    &paint,
                );
                canvas.restore();
                canvas.translate((size + margin, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, size + margin));
        }
    }
}

// Port of: gm/imagefilterstransformed.cpp#L115 (chrome/m156), DEF_GM( return new ImageFiltersTransformedGM; )
crate::def_gm!(
    ImageFiltersTransformedGM,
    ImageFiltersTransformedGm {
        checkerboard: None,
        gradient_circle: None
    }
);

// Port of: gm/imagefilterstransformed.cpp#L136-L179 (chrome/m156), rotate_imagefilter
crate::def_simple_gm!(rotate_imagefilter, canvas, 500, 500, {
    let r = Rect::from_xywh(50.0, 50.0, 100.0, 100.0);
    let filters: [Option<ImageFilter>; 3] = [
        None,
        blur(6.0, 0.0, TileMode::Decal, None, None),
        blend(BlendMode::SrcOver, None, None, None),
    ];
    let mut paint = Paint::default();
    for filter in filters {
        paint.set_anti_alias(false);
        paint.set_image_filter(filter);
        canvas.save();
        canvas.draw_rect(r, &paint);
        canvas.translate((150.0, 0.0));
        canvas.save();
        canvas.rotate(30.0, Some(Point::new(100.0, 100.0)));
        canvas.draw_rect(r, &paint);
        canvas.restore();
        paint.set_anti_alias(true);
        canvas.translate((150.0, 0.0));
        canvas.save();
        canvas.rotate(30.0, Some(Point::new(100.0, 100.0)));
        canvas.draw_rect(r, &paint);
        canvas.restore();
        canvas.restore();
        canvas.translate((0.0, 150.0));
    }
});

// Port of: gm/imagefilterstransformed.cpp#L229-L314 (chrome/m156), ImageFilterComposedTransform
struct ImageFilterComposedTransformGm {
    image: Option<Image>,
}

impl ImageFilterComposedTransformGm {
    // Port of: gm/imagefilterstransformed.cpp#L262-L273 (chrome/m156), drawFilter
    fn draw_filter(canvas: &Canvas, tx: f32, ty: f32, filter: Option<ImageFilter>, image: &Image) {
        let mut p = Paint::default();
        p.set_image_filter(filter);
        canvas.save();
        canvas.translate((tx, ty));
        canvas.clip_rect(Rect::from_wh(256.0, 256.0), None, None);
        canvas.scale((0.5, 0.5));
        canvas.translate((128.0, 128.0));
        canvas.draw_image_with_sampling_options(
            image,
            (0.0, 0.0),
            SamplingOptions::from(FilterMode::Linear),
            Some(&p),
        );
        canvas.restore();
    }

    // Port of: gm/imagefilterstransformed.cpp#L276-L284 (chrome/m156), makeDirectFilter
    fn make_direct_filter(matrix: &Matrix, image: &Image) -> Option<ImageFilter> {
        let v = (image.width() as f32 / 2.0, image.height() as f32 / 2.0);
        let filter = offset((-v.0, -v.1), None, None);
        let filter = matrix_transform(
            matrix,
            SamplingOptions::from(FilterMode::Linear),
            filter,
        );
        offset((v.0, v.1), filter, None)
    }

    // Port of: gm/imagefilterstransformed.cpp#L287-L296 (chrome/m156), makeEarlyComposeFilter
    fn make_early_compose_filter(matrix: &Matrix, image: &Image) -> Option<ImageFilter> {
        let v = (image.width() as f32 / 2.0, image.height() as f32 / 2.0);
        let offset_filter = offset((-v.0, -v.1), None, None);
        let filter = matrix_transform(matrix, SamplingOptions::from(FilterMode::Linear), None);
        let filter = compose(filter, offset_filter);
        offset((v.0, v.1), filter, None)
    }

    // Port of: gm/imagefilterstransformed.cpp#L299-L307 (chrome/m156), makeLateComposeFilter
    fn make_late_compose_filter(matrix: &Matrix, image: &Image) -> Option<ImageFilter> {
        let v = (image.width() as f32 / 2.0, image.height() as f32 / 2.0);
        let filter = offset((-v.0, -v.1), None, None);
        let filter = matrix_transform(matrix, SamplingOptions::from(FilterMode::Linear), filter);
        let offset_filter = offset((v.0, v.1), None, None);
        compose(offset_filter, filter)
    }

    // Port of: gm/imagefilterstransformed.cpp#L310-L320 (chrome/m156), makeFullComposeFilter
    fn make_full_compose_filter(matrix: &Matrix, image: &Image) -> Option<ImageFilter> {
        let v = (image.width() as f32 / 2.0, image.height() as f32 / 2.0);
        let offset_filter = offset((-v.0, -v.1), None, None);
        let filter = matrix_transform(matrix, SamplingOptions::from(FilterMode::Linear), None);
        let filter = compose(filter, offset_filter);
        let offset_filter = offset((v.0, v.1), None, None);
        compose(offset_filter, filter)
    }
}

impl GM for ImageFilterComposedTransformGm {
    fn name(&self) -> String {
        "imagefilter_composed_transform".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(512, 512)
    }

    fn on_once_before_draw(&mut self) {
        self.image = get_resource_as_image("images/mandrill_256.png");
    }

    // Port of: gm/imagefilterstransformed.cpp#L243-L253 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(image) = self.image.clone() else {
            return;
        };
        // Start at 70 degrees since that highlighted the issue in skbug.com/40042261
        let matrix = Matrix::rotate_deg(70.0);
        // All four quadrants should render the same
        Self::draw_filter(
            canvas,
            0.0,
            0.0,
            Self::make_direct_filter(&matrix, &image),
            &image,
        );
        Self::draw_filter(
            canvas,
            256.0,
            0.0,
            Self::make_early_compose_filter(&matrix, &image),
            &image,
        );
        Self::draw_filter(
            canvas,
            0.0,
            256.0,
            Self::make_late_compose_filter(&matrix, &image),
            &image,
        );
        Self::draw_filter(
            canvas,
            256.0,
            256.0,
            Self::make_full_compose_filter(&matrix, &image),
            &image,
        );
    }
}

// Port of: gm/imagefilterstransformed.cpp#L315 (chrome/m156), DEF_GM(return new ImageFilterComposedTransform();)
crate::def_gm!(
    ImageFilterComposedTransform_ = "ImageFilterComposedTransform()",
    ImageFilterComposedTransformGm { image: None }
);
