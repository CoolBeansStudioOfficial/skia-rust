// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/ToolUtils.cpp

//! The parts of `ToolUtils` that GMs use.

use skia_rust_core::color::{Color, pre_multiply_color};
use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_PI, scalar_cos, scalar_sin};

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
