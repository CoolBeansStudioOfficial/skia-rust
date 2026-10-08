// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmaptiled.cpp (chrome/m156)
//
// `bitmaptiled_fractional_horizontal` and `bitmaptiled_fractional_vertical` are Ganesh-only
// (excluded).

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tiled_image_utils;

// This test exercises Ganesh's drawing of tiled bitmaps. In particular, that the offsets and the
// extents of the tiles don't cause gaps between tiles.
// Port of: gm/bitmaptiled.cpp#L22-L68 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // i * 0.1f, (kTileSize - 50) + offset, 37.0f * i
fn draw_tile_bitmap_with_fractional_offset(canvas: &Canvas, vertical: bool, manual: bool) {
    // This should match kBmpSmallTileSize in SkGpuDevice.cpp. Note that our canvas size is tuned
    // to this constant as well.
    const TILE_SIZE: i32 = 1 << 10;

    // We're going to draw a section of the bitmap that intersects 3 tiles (3x1 or 1x3).
    // We need that to be < 50% of the total image, so our image is 7 tiles (7x1 or 1x7).
    const BITMAP_LONG_EDGE: i32 = 7 * TILE_SIZE;
    const BITMAP_SHORT_EDGE: i32 = TILE_SIZE;

    // (The Ganesh resource cache limit is not set: there is no GPU context.)

    // Construct our bitmap as either very wide or very tall
    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels(
        if vertical {
            (BITMAP_SHORT_EDGE, BITMAP_LONG_EDGE)
        } else {
            (BITMAP_LONG_EDGE, BITMAP_SHORT_EDGE)
        },
        true,
    );
    bmp.erase_color(Color::WHITE);

    // Draw ten strips with varying fractional offset to catch any rasterization issues with tiling
    for i in 0..10 {
        let offset = i as f32 * 0.1;

        let src = if vertical {
            Rect::from_xywh(0.0, (TILE_SIZE - 50) as f32 + offset, 32.0, 1124.0)
        } else {
            Rect::from_xywh((TILE_SIZE - 50) as f32 + offset, 0.0, 1124.0, 32.0)
        };
        let dst = if vertical {
            Rect::from_xywh(37.0 * i as f32, 0.0, 32.0, 1124.0)
        } else {
            Rect::from_xywh(0.0, 37.0 * i as f32, 1124.0, 32.0)
        };

        let image = bmp.as_image().expect("an image");
        if manual {
            tiled_image_utils::draw_image_rect(
                canvas,
                &image,
                &src,
                &dst,
                &SamplingOptions::default(),
                /* paint= */ None,
                SrcRectConstraint::Strict,
            );
        } else {
            // (`paint= nullptr` is the default paint.)
            canvas.draw_image_rect(
                &image,
                Some((&src, SrcRectConstraint::Strict)),
                dst,
                &Paint::default(),
            );
        }
    }
}

// Port of: gm/bitmaptiled.cpp#L84-L86 (chrome/m156)
crate::def_simple_gm_bg!(
    bitmaptiled_fractional_horizontal_manual,
    canvas,
    1124,
    365,
    Color::BLACK,
    {
        draw_tile_bitmap_with_fractional_offset(
            canvas, /* vertical= */ false, /* manual= */ true,
        );
    }
);

// Port of: gm/bitmaptiled.cpp#L87-L89 (chrome/m156)
crate::def_simple_gm_bg!(
    bitmaptiled_fractional_vertical_manual,
    canvas,
    365,
    1124,
    Color::BLACK,
    {
        draw_tile_bitmap_with_fractional_offset(
            canvas, /* vertical= */ true, /* manual= */ true,
        );
    }
);
