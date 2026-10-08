// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fatpathfill.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_raster::surfaces;

const ZOOM: i32 = 32;
const SMALL_W: i32 = 9;
const SMALL_H: i32 = 3;
const REPEAT_LOOP: i32 = 5;

// Port of: gm/fatpathfill.cpp#L28-L30 (chrome/m156)
fn new_surface(width: i32, height: i32) -> Option<Surface<'static>> {
    surfaces::raster(
        &ImageInfo::new_n32_premul((width, height), None),
        None,
        None,
    )
}

// Port of: gm/fatpathfill.cpp#L32-L42 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // x + 0.5f, 1.5f / ZOOM
fn draw_pixel_centers(canvas: &Canvas) {
    let mut paint = Paint::default();
    paint.set_color(tool_utils::color_to_565(Color::new(0xFF00_88FF)));
    paint.set_anti_alias(true);

    for y in 0..SMALL_H {
        for x in 0..SMALL_W {
            canvas.draw_circle((x as f32 + 0.5, y as f32 + 0.5), 1.5 / ZOOM as f32, &paint);
        }
    }
}

// Port of: gm/fatpathfill.cpp#L44-L57 (chrome/m156)
fn draw_fatpath(canvas: &Canvas, surface: &mut Surface<'_>, path: &Path) {
    let mut paint = Paint::default();

    surface.canvas().clear(Color::TRANSPARENT);
    surface.canvas().draw_path(path, &paint);
    surface.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);

    paint.set_anti_alias(true);
    paint.set_color(Color::RED);
    paint.set_style(Style::Stroke);
    canvas.draw_path(path, &paint);

    draw_pixel_centers(canvas);
}

// Port of: gm/fatpathfill.cpp#L59-L78 (chrome/m156)
crate::def_simple_gm!(
    fatpathfill,
    canvas,
    SMALL_W * ZOOM,
    SMALL_H * ZOOM * REPEAT_LOOP,
    {
        let mut surface = new_surface(SMALL_W, SMALL_H).expect("a surface");

        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        canvas.scale((ZOOM as f32, ZOOM as f32));

        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(1.0);

        for i in 0..REPEAT_LOOP {
            #[allow(clippy::cast_precision_loss)] // SkIntToScalar
            let line = Path::line((1.0, 2.0), ((4 + i) as f32, 1.0));
            let mut builder = PathBuilder::new();
            fill_path_with_paint(&line, &paint, &mut builder, None, None);
            draw_fatpath(canvas, &mut surface, &builder.detach());

            #[allow(clippy::cast_precision_loss)] // SkIntToScalar
            canvas.translate((0.0, SMALL_H as f32));
        }
    }
);
