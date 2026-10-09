// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/localmatriximagefilter.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::make_surface;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, dilate, erode, offset};

// Port of: gm/localmatriximagefilter.cpp#L20-L31 (chrome/m156), make_image
fn make_image(root_canvas: &Canvas) -> Option<Image> {
    let info = ImageInfo::new((100, 100), ColorType::N32, AlphaType::Premul, None);
    let mut surface = make_surface(root_canvas, &info, None)?;
    {
        let canvas = surface.canvas();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::RED);
        canvas.draw_circle((50.0, 50.0), 50.0, &paint);
    }
    surface.image_snapshot()
}

// Port of: gm/localmatriximagefilter.cpp#L33-L43 (chrome/m156), show_image
fn show_image(canvas: &Canvas, image: &Image, filter: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    // SkRect::MakeIWH(w, h).makeOutset(SK_ScalarHalf, SK_ScalarHalf)
    let r = Rect::from_ltrb(
        -0.5,
        -0.5,
        image.width() as f32 + 0.5,
        image.height() as f32 + 0.5,
    );
    canvas.draw_rect(r, &paint);

    paint.set_style(Style::Fill);
    paint.set_image_filter(filter);
    canvas.draw_image_with_sampling_options(
        image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&paint),
    );
}

// Port of: gm/localmatriximagefilter.cpp#L46-L83 (chrome/m156), localmatriximagefilter
crate::def_simple_gm!(localmatriximagefilter, canvas, 640, 640, {
    let Some(image0) = make_image(canvas) else {
        return;
    };

    // Each factory is `SkImageFilters::{Blur,Dilate,Erode,Offset}(8, 8, nullptr)`.
    let factories: [fn() -> Option<ImageFilter>; 4] = [
        || blur(8.0, 8.0, TileMode::Decal, None, None),
        || dilate((8.0, 8.0), None, None),
        || erode((8.0, 8.0), None, None),
        || offset((8.0, 8.0), None, None),
    ];

    let matrices = [
        Matrix::scale((0.5, 0.5)),
        Matrix::scale((2.0, 2.0)),
        Matrix::translate((10.0, 10.0)),
    ];

    let spacer = image0.width() as f32 * 3.0 / 2.0;

    canvas.translate((40.0, 40.0));
    for factory in factories {
        let filter = factory();

        canvas.save();
        show_image(canvas, &image0, filter.clone());
        for matrix in &matrices {
            let local_filter = filter.as_ref().and_then(|f| f.with_local_matrix(matrix));
            canvas.translate((spacer, 0.0));
            show_image(canvas, &image0, local_filter);
        }
        canvas.restore();
        canvas.translate((0.0, spacer));
    }
});
