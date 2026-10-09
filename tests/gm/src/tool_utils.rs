// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/ToolUtils.cpp

//! The parts of `ToolUtils` that GMs use.

use std::path::PathBuf;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::{Color, pre_multiply_color};
use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
use skia_rust_core::data::Data;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::IRect;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{SCALAR_PI, scalar_cos, scalar_sin};
use skia_rust_core::shader::Shader;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

/// `(float)v` for a GM coordinate or size: the small integers GMs use are exact in a float.
// mirrors the implicit int-to-float conversions of the C++ GM code
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn int_to_scalar(v: i32) -> f32 {
    v as f32
}

/// Port of `GetResourcePath`: the directory of Skia's `resources` (the `SKIA_RESOURCES`
/// environment variable, else this workspace's pinned Skia checkout), if it exists. The same
/// lookup as `tests/src/resources.rs`, which this crate cannot depend on.
// Port of: tools/Resources.cpp#L23-L25 (chrome/m156)
#[must_use]
pub fn resource_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("SKIA_RESOURCES") {
        candidates.push(PathBuf::from(dir));
    }
    candidates.push(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("third_party")
            .join("skia")
            .join("resources"),
    );
    candidates.into_iter().find(|dir| dir.is_dir())
}

/// Port of `GetResourceAsData`: the contents of the resource at `path` (relative to Skia's
/// `resources` directory, using `/` separators), or `None` if it cannot be read.
// Port of: tools/Resources.cpp#L42-L50 (chrome/m156)
#[must_use]
pub fn get_resource_as_data(path: &str) -> Option<Vec<u8>> {
    let mut full = resource_dir()?;
    for component in path.split('/') {
        full.push(component);
    }
    std::fs::read(full).ok()
}

/// Port of `ToolUtils::GetResourceAsImage`: a lazy image of the encoded resource at `path`
/// (`SkImages::DeferredFromEncodedData(GetResourceAsData(path))`).
// Port of: tools/DecodeUtils.h#L31-L33 (chrome/m156)
#[must_use]
pub fn get_resource_as_image(path: &str) -> Option<Image> {
    let data = get_resource_as_data(path)?;
    skia_rust_codec::images::deferred_from_encoded_data(Some(Data::new_from_vec(data)), None)
}

/// `ToolUtils::color_to_565`: rounds `color` to what a 565 surface would store.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
#[must_use]
pub fn color_to_565(color: impl Into<Color>) -> Color {
    let color = color.into();
    // Not a good idea to use this function for greyscale colors...
    // it will add an obvious purple or green tint.
    debug_assert!(color.r() != color.g() || color.r() != color.b() || color.g() != color.b());

    let pm_color = pre_multiply_color(color);
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

/// `ToolUtils::make_star`: a star polygon with `num_pts` points, stepping `step` points at a
/// time, fitted into `bounds`.
// Port of: tools/ToolUtils.cpp#L271-L285 (chrome/m156)
#[must_use]
#[allow(clippy::cast_precision_loss)] // int * SkScalar arithmetic as in C++
pub fn make_star(bounds: &Rect, num_pts: i32, step: i32) -> Path {
    debug_assert_ne!(num_pts, step);
    let mut builder = PathBuilder::new();
    builder.set_fill_type(PathFillType::EvenOdd);
    builder.move_to((0.0, -1.0));
    for i in 1..num_pts {
        let idx = i * step % num_pts;
        let theta: f32 = idx as f32 * 2.0 * SCALAR_PI / num_pts as f32 + SCALAR_PI / 2.0;
        let x: f32 = scalar_cos(theta);
        let y: f32 = -scalar_sin(theta);
        builder.line_to((x, y));
    }
    let path = builder.detach();
    path.make_transform(&Matrix::rect_to_rect_or_identity(
        path.bounds(),
        bounds,
        None,
    ))
}

/// `ToolUtils::create_checkerboard_shader`: a repeating `2 * size` checkerboard of `c1` and `c2`.
// Port of: tools/ToolUtils.cpp#L153-L160 (chrome/m156)
#[must_use]
pub fn create_checkerboard_shader(c1: Color, c2: Color, size: i32) -> Option<Shader> {
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(
        &ImageInfo::new_s32((2 * size, 2 * size), AlphaType::Premul),
        None,
    );
    bm.erase_color(c1);
    bm.erase_area(IRect::new(0, 0, size, size), c2);
    bm.erase_area(IRect::new(size, size, 2 * size, 2 * size), c2);
    bm.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        None,
    )
}

/// `ToolUtils::create_checkerboard_bitmap`: a `w` by `h` sRGB bitmap with a checkerboard of
/// `c1` and `c2` squares of `check_size`.
///
/// # Panics
/// If the pixels cannot be allocated.
// Port of: tools/ToolUtils.cpp#L162-L169 (chrome/m156)
#[must_use]
pub fn create_checkerboard_bitmap(w: i32, h: i32, c1: Color, c2: Color, check_size: i32) -> Bitmap {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&ImageInfo::new_s32((w, h), AlphaType::Premul), None);
    {
        let canvas = Canvas::from_bitmap(&mut bitmap, None).expect("a canvas");
        draw_checkerboard(&canvas, c1, c2, check_size);
    }
    bitmap
}

/// `ToolUtils::draw_checkerboard`: fills `canvas` with a checkerboard of `c1` and `c2` squares of
/// `size`.
// Port of: tools/ToolUtils.cpp#L177-L182 (chrome/m156)
pub fn draw_checkerboard(canvas: &Canvas, c1: Color, c2: Color, size: i32) {
    let mut paint = Paint::default();
    paint.set_shader(create_checkerboard_shader(c1, c2, size));
    paint.set_blend_mode(BlendMode::Src);
    canvas.draw_paint(&paint);
}

/// `ToolUtils::makeSurface`: a surface compatible with `canvas`, or a raster one.
// Port of: tools/ToolUtils.cpp#L504-L512 (chrome/m156)
#[must_use]
pub fn make_surface(
    canvas: &Canvas,
    info: &ImageInfo,
    props: Option<&SurfaceProps>,
) -> Option<Surface<'static>> {
    canvas
        .new_surface(info, props)
        .or_else(|| surfaces::raster(info, None, props))
}
