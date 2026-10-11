// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colorfilterimagefilter.cpp (chrome/m156)
//
// Not ported: `colorfiltershader` (it needs the `images/mandrill_128.png` resource).

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{AutoCanvasRestore, SaveLayerRec};
use skia_rust_core::color::colors;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_effects::image_filters::{blur, color_filter};

use crate::tool_utils::get_resource_as_image;

const FILTER_WIDTH: f32 = 30.0;
const FILTER_HEIGHT: f32 = 30.0;
const MARGIN: f32 = 10.0;

// Port of: gm/colorfilterimagefilter.cpp#L17-L27 (chrome/m156)
fn cf_make_brightness(brightness: f32, clamp: Clamp) -> Option<ColorFilter> {
    let matrix: [f32; 20] = [
        1.0, 0.0, 0.0, 0.0, brightness, //
        0.0, 1.0, 0.0, 0.0, brightness, //
        0.0, 0.0, 1.0, 0.0, brightness, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    color_filters::matrix_row_major(&matrix, clamp)
}

// Port of: gm/colorfilterimagefilter.cpp#L29-L38 (chrome/m156)
fn cf_make_grayscale() -> Option<ColorFilter> {
    let mut matrix = [0.0f32; 20];
    matrix[0] = 0.2126;
    matrix[5] = 0.2126;
    matrix[10] = 0.2126;
    matrix[1] = 0.7152;
    matrix[6] = 0.7152;
    matrix[11] = 0.7152;
    matrix[2] = 0.0722;
    matrix[7] = 0.0722;
    matrix[12] = 0.0722;
    matrix[18] = 1.0;
    color_filters::matrix_row_major(&matrix, Clamp::Yes)
}

// Port of: gm/colorfilterimagefilter.cpp#L40-L42 (chrome/m156)
fn cf_make_colorize(color: Color) -> Option<ColorFilter> {
    color_filters::blend_color(color, BlendMode::Src)
}

// Port of: gm/colorfilterimagefilter.cpp#L56-L58 (chrome/m156)
fn make_blur(amount: f32, input: Option<ImageFilter>) -> Option<ImageFilter> {
    blur(
        amount,
        amount,
        skia_rust_core::tile_mode::TileMode::Decal,
        input,
        None,
    )
}

// Port of: gm/colorfilterimagefilter.cpp#L60-L63 (chrome/m156)
fn make_brightness(amount: f32, input: Option<ImageFilter>, clamp: Clamp) -> Option<ImageFilter> {
    color_filter(cf_make_brightness(amount, clamp), input, None)
}

// Port of: gm/colorfilterimagefilter.cpp#L64-L66 (chrome/m156)
fn make_grayscale(input: Option<ImageFilter>) -> Option<ImageFilter> {
    color_filter(cf_make_grayscale(), input, None)
}

// Port of: gm/colorfilterimagefilter.cpp#L67-L69 (chrome/m156)
fn make_mode_blue(input: Option<ImageFilter>) -> Option<ImageFilter> {
    color_filter(cf_make_colorize(Color::BLUE), input, None)
}

// Port of: gm/colorfilterimagefilter.cpp#L71-L80 (chrome/m156)
fn draw_clipped_rect(canvas: &Canvas, r: Rect, paint: &Paint, outset: f32) {
    let guard = AutoCanvasRestore::guard(canvas, true);
    let mut clip = r;
    clip.outset((outset, outset));
    guard.clip_rect(clip, None, None);
    guard.draw_rect(r, paint);
}

// Port of: gm/colorfilterimagefilter.cpp#L83-L127 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the GM's onDraw
fn draw_colorfilterimagefilter(canvas: &Canvas) {
    let r = Rect::from_wh(FILTER_WIDTH, FILTER_HEIGHT);
    let mut paint = Paint::default();
    paint.set_color(Color::RED);
    canvas.save();
    let mut brightness = -1.0f32;
    while brightness <= 1.0 {
        let dim = make_brightness(-brightness, None, Clamp::Yes);
        let bright = make_brightness(brightness, dim, Clamp::Yes);
        paint.set_image_filter(bright);
        draw_clipped_rect(canvas, r, &paint, 0.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
        brightness += 0.2;
    }
    canvas.restore();
    canvas.translate((0.0, FILTER_HEIGHT + MARGIN));
    canvas.save();
    let mut brightness = -1.0f32;
    while brightness <= 1.0 {
        let dim = make_brightness(-brightness, None, Clamp::No);
        let bright = make_brightness(brightness, dim, Clamp::No);
        paint.set_image_filter(bright);
        draw_clipped_rect(canvas, r, &paint, 0.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
        brightness += 0.2;
    }
    canvas.restore();
    canvas.translate((0.0, FILTER_HEIGHT + MARGIN));
    {
        let brightness = make_brightness(0.9, None, Clamp::Yes);
        let grayscale = make_grayscale(brightness);
        paint.set_image_filter(grayscale);
        draw_clipped_rect(canvas, r, &paint, 0.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
    }
    {
        let grayscale = make_grayscale(None);
        let brightness = make_brightness(0.9, grayscale, Clamp::Yes);
        paint.set_image_filter(brightness);
        draw_clipped_rect(canvas, r, &paint, 0.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
    }
    {
        let blue = make_mode_blue(None);
        let brightness = make_brightness(1.0, blue, Clamp::Yes);
        paint.set_image_filter(brightness);
        draw_clipped_rect(canvas, r, &paint, 0.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
    }
    {
        let brightness = make_brightness(1.0, None, Clamp::Yes);
        let blue = make_mode_blue(brightness);
        paint.set_image_filter(blue);
        draw_clipped_rect(canvas, r, &paint, 0.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
    }
    {
        let blur_filter = make_blur(3.0, None);
        let brightness = make_brightness(0.5, blur_filter, Clamp::Yes);
        paint.set_image_filter(brightness);
        draw_clipped_rect(canvas, r, &paint, 3.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
    }
    {
        let blue = make_mode_blue(None);
        paint.set_image_filter(blue);
        draw_clipped_rect(canvas, r, &paint, 5.0);
        canvas.translate((FILTER_WIDTH + MARGIN, 0.0));
    }
    canvas.restore();
}

crate::def_simple_gm!(colorfilterimagefilter, canvas, 435, 120, {
    draw_colorfilterimagefilter(canvas);
});

// Port of: gm/colorfilterimagefilter.cpp#L129-L139 (chrome/m156)
crate::def_simple_gm!(colorfilterimagefilter_layer, canvas, 32, 32, {
    let guard = AutoCanvasRestore::guard(canvas, false);
    let mut cm = ColorMatrix::default();
    cm.set_saturation(0.0);
    let cf = color_filters::matrix(&cm, Clamp::Yes);
    let mut p = Paint::default();
    p.set_image_filter(color_filter(cf, None, None));
    guard.save_layer(&SaveLayerRec::default().paint(&p));
    guard.clear(Color::RED);
});

// Port of: gm/colorfilterimagefilter.cpp#L86-L89 (chrome/m156), sh_make_lineargradient0
fn sh_make_lineargradient0() -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
    let colors = [colors::RED, colors::GREEN, colors::BLUE];
    shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Repeat, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/colorfilterimagefilter.cpp#L91-L97 (chrome/m156), sh_make_lineargradient1
fn sh_make_lineargradient1() -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
    let colors = [colors::RED, Color4f::new(0.0, 1.0, 0.0, 0.0), colors::BLUE];
    shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Repeat, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/colorfilterimagefilter.cpp#L99-L105 (chrome/m156), sh_make_image
fn sh_make_image() -> Option<Shader> {
    let image = get_resource_as_image("images/mandrill_128.png")?;
    image.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        None,
    )
}

// Port of: gm/colorfilterimagefilter.cpp#L107-L122 (chrome/m156), sk_gm_get_shaders
fn sk_gm_get_shaders() -> Vec<Shader> {
    let mut array = Vec::new();
    array.extend(sh_make_lineargradient0());
    array.extend(sh_make_lineargradient1());
    array.extend(sh_make_image());
    array
}

// Port of: gm/colorfilterimagefilter.cpp#L63-L68 (chrome/m156), sk_gm_get_colorfilters
fn sk_gm_get_colorfilters() -> Vec<ColorFilter> {
    let mut array = Vec::new();
    array.extend(cf_make_brightness(0.5, Clamp::Yes));
    array.extend(cf_make_grayscale());
    array.extend(cf_make_colorize(Color::BLUE));
    array
}

// Port of: gm/colorfilterimagefilter.cpp#L217-L247 (chrome/m156), colorfiltershader
crate::def_simple_gm!(colorfiltershader, canvas, 610, 610, {
    let filters = sk_gm_get_colorfilters();

    let mut shaders_list = sk_gm_get_shaders();
    let colors = [colors::RED, colors::BLUE];
    shaders_list.extend(shaders::two_point_conical_gradient(
        ((0.0, 0.0), 50.0),
        ((0.0, 0.0), 150.0),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));

    let mut paint = Paint::default();
    let r = Rect::from_wh(120.0, 120.0);

    canvas.translate((20.0, 20.0));
    for shader in &shaders_list {
        canvas.save();
        // `None` is the C++ `nullptr` filter: the shader is used unchanged.
        let filter_iter = std::iter::once(None).chain(filters.iter().cloned().map(Some));
        for filter in filter_iter {
            paint.set_shader(match filter {
                Some(f) => shader.with_color_filter(f),
                None => shader.clone(),
            });
            canvas.draw_rect(r, &paint);
            canvas.translate((150.0, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, 150.0));
    }
});
