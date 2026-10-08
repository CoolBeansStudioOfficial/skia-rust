// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkTiledImageUtils.h, src/image/SkTiledImageUtils.cpp

//! `SkTiledImageUtils`: draws that break bitmap-backed images into tiles when they are too large
//! to upload to the GPU, and otherwise fall through to the matching [`Canvas`] call.

use crate::canvas::{Canvas, SrcRectConstraint};
use crate::image::Image;
use crate::paint::Paint;
use crate::rect::Rect;
use crate::sampling_options::SamplingOptions;

/// Draws the `src` rect of `image` into `dst`, tiling the image if the device needs it
/// (`SkTiledImageUtils::DrawImageRect`).
///
/// skia-rust: only the raster device exists, whose `drawAsTiledImageRect` always returns false,
/// so this is the default `drawImageRect`.
// Port of: src/image/SkTiledImageUtils.cpp#L19-L39 (chrome/m156)
#[doc(alias = "SkTiledImageUtils::DrawImageRect")]
pub fn draw_image_rect(
    canvas: &Canvas,
    image: &Image,
    src: &Rect,
    dst: &Rect,
    sampling: &SamplingOptions,
    paint: Option<&Paint>,
    constraint: SrcRectConstraint,
) {
    // (`SkCanvasPriv::TopDevice(canvas)->drawAsTiledImageRect(...)` is false on a raster
    // device.) Either the image didn't require tiling or this is a raster-backed
    // canvas. In either case fall back to a default draw.
    canvas.draw_image_rect_nullable_paint(image, src, dst, sampling, paint, constraint);
}

/// Draws `image` with its top-left corner at `(x, y)`, tiling it if the device needs it
/// (`SkTiledImageUtils::DrawImage`).
// Port of: include/core/SkTiledImageUtils.h#L78-L93 (chrome/m156)
#[doc(alias = "SkTiledImageUtils::DrawImage")]
#[allow(clippy::cast_precision_loss)] // SkRect::MakeIWH, MakeXYWH(x, y, int, int)
pub fn draw_image(
    canvas: &Canvas,
    image: &Image,
    x: f32,
    y: f32,
    sampling: &SamplingOptions,
    paint: Option<&Paint>,
    constraint: SrcRectConstraint,
) {
    let src = Rect::from_wh(image.width() as f32, image.height() as f32);
    let dst = Rect::from_xywh(x, y, image.width() as f32, image.height() as f32);

    draw_image_rect(canvas, image, &src, &dst, sampling, paint, constraint);
}
