// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Crate tests for the raster path masks of `graphite::raster_path_utils`: the padded A8 mask
// allocation, the CPU coverage drawn into it, and the key that identifies a rasterized mask.

use skia_rust_core::m44::M44;
use skia_rust_core::point::IPoint;
use skia_rust_core::size::ISize;
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_gpu::graphite::geom::rect::Rect;
use skia_rust_gpu::graphite::geom::shape::Shape;
use skia_rust_gpu::graphite::geom::transform::Transform;
use skia_rust_gpu::graphite::raster_path_utils::{RasterMaskHelper, generate_path_mask_key};

const PADDING_ALPHA: u8 = 0xAA;

fn identity() -> Transform {
    Transform::new(M44::new_identity())
}

// The pixel at (x, y) of the padded buffer, with (0, 0) the top-left of the padding.
fn at(buffer: &skia_rust_gpu::graphite::raster_path_utils::MaskBuffer, x: usize, y: usize) -> u8 {
    buffer.pixels()[y * buffer.row_bytes() + x]
}

#[test]
fn allocate_fills_padding_and_inner_with_the_initial_alpha() {
    let buffer = RasterMaskHelper::allocate(ISize::new(4, 3), 1, PADDING_ALPHA);
    assert_eq!(buffer.size(), ISize::new(4, 3));
    assert_eq!(buffer.padding(), 1);
    assert_eq!(buffer.row_bytes(), 6);
    assert_eq!(buffer.pixels().len(), 6 * 5);
    assert!(buffer.pixels().iter().all(|&a| a == PADDING_ALPHA));
}

#[test]
fn filled_rect_covers_its_pixels_and_leaves_the_padding_alone() {
    let mut buffer = RasterMaskHelper::allocate(ISize::new(8, 8), 1, PADDING_ALPHA);
    {
        let mut helper = RasterMaskHelper::over(&mut buffer, IPoint::new(0, 0));
        let shape = Shape::from_rect(Rect::new(2.0, 2.0, 6.0, 6.0));
        helper.draw_shape(&shape, &identity(), &StrokeRec::new_fill());
    }
    // Inner pixel (x, y) is buffer pixel (x + 1, y + 1).
    assert_eq!(at(&buffer, 1 + 3, 1 + 3), 255, "interior is fully covered");
    assert_eq!(
        at(&buffer, 1 + 2, 1 + 2),
        255,
        "the rect's top-left pixel is covered"
    );
    assert_eq!(
        at(&buffer, 1 + 5, 1 + 5),
        255,
        "the rect's bottom-right pixel is covered"
    );
    // The coverage draw writes only the pixels inside the path's scan bounds (as SkDraw does), so
    // pixels outside them keep the initial alpha. `RasterMaskHelper::allocate` zeroes the mask
    // when the caller needs zero coverage there.
    assert_eq!(
        at(&buffer, 1, 1),
        PADDING_ALPHA,
        "outside the path's bounds: untouched"
    );
    assert_eq!(
        at(&buffer, 1 + 6, 1 + 6),
        PADDING_ALPHA,
        "past the path's bounds: untouched"
    );
    // The padding is outside the inner region, which is all the draw can reach.
    assert_eq!(at(&buffer, 0, 0), PADDING_ALPHA);
    assert_eq!(at(&buffer, 9, 9), PADDING_ALPHA);
    assert_eq!(at(&buffer, 0, 5), PADDING_ALPHA);
}

#[test]
fn fractional_edges_have_partial_coverage() {
    let mut buffer = RasterMaskHelper::allocate(ISize::new(8, 8), 1, 0);
    {
        let mut helper = RasterMaskHelper::over(&mut buffer, IPoint::new(0, 0));
        let shape = Shape::from_rect(Rect::new(2.5, 2.5, 5.5, 5.5));
        helper.draw_shape(&shape, &identity(), &StrokeRec::new_fill());
    }
    let corner = at(&buffer, 1 + 2, 1 + 2);
    assert!(
        corner > 0 && corner < 255,
        "a pixel the rect covers by a quarter is partially covered, got {corner}"
    );
    assert_eq!(
        at(&buffer, 1 + 3, 1 + 3),
        255,
        "the interior is fully covered"
    );
    assert_eq!(
        at(&buffer, 1 + 1, 1 + 1),
        0,
        "outside the rect the coverage is zero"
    );
}

#[test]
fn clip_alpha_scales_the_coverage() {
    let mut buffer = RasterMaskHelper::allocate(ISize::new(8, 8), 0, 0);
    {
        let mut helper = RasterMaskHelper::over(&mut buffer, IPoint::new(0, 0));
        let shape = Shape::from_rect(Rect::new(0.0, 0.0, 8.0, 8.0));
        helper.draw_clip(&shape, &identity(), 0x80);
    }
    assert_eq!(
        at(&buffer, 3, 3),
        0x80,
        "a full-coverage clip at half alpha is 0x80"
    );
}

#[test]
fn path_mask_key_identifies_the_mask() {
    let shape = Shape::from_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
    let fill = StrokeRec::new_fill();
    let key = generate_path_mask_key(&shape, &identity(), &fill, (0, 0), (10, 10));
    assert!(key.is_valid());
    assert_eq!(
        key,
        generate_path_mask_key(&shape, &identity(), &fill, (0, 0), (10, 10)),
        "the same mask has the same key"
    );
    assert_ne!(
        key,
        generate_path_mask_key(&shape, &identity(), &fill, (1, 0), (10, 10)),
        "the mask origin is part of the key"
    );
    assert_ne!(
        key,
        generate_path_mask_key(&shape, &identity(), &fill, (0, 0), (11, 10)),
        "the mask size is part of the key"
    );
    let shifted = Transform::new(M44::translate(0.5, 0.0, 0.0));
    assert_ne!(
        key,
        generate_path_mask_key(&shape, &shifted, &fill, (0, 0), (10, 10)),
        "the sub-pixel translation is part of the key"
    );
    let hairline = StrokeRec::new_hairline();
    assert_ne!(
        key,
        generate_path_mask_key(&shape, &identity(), &hairline, (0, 0), (10, 10)),
        "the style is part of the key"
    );
}
