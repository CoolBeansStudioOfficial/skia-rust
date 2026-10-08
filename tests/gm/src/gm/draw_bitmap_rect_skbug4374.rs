// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/draw_bitmap_rect_skbug4374.cpp (chrome/m156)

use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;

use crate::tool_utils::{get_resource_as_image, int_to_scalar};

// Port of: gm/draw_bitmap_rect_skbug4374.cpp#L19-L30 (chrome/m156)
// (The GM's name is `draw_bitmap_rect_skbug4734`, as Skia registers it.)
crate::def_simple_gm!(draw_bitmap_rect_skbug4734, canvas, 64, 64, {
    // ToolUtils::MakeTextureImage returns the image unchanged on a raster canvas.
    if let Some(img) = get_resource_as_image("images/randPixels.png") {
        let mut rect = Rect::from_wh(int_to_scalar(img.width()), int_to_scalar(img.height()));
        rect.inset((0.5, 1.5));
        let (dst, _) = Matrix::scale((8.0, 8.0)).map_rect(rect);
        canvas.draw_image_rect_with_sampling_options(
            &img,
            Some((&rect, SrcRectConstraint::Strict)),
            dst,
            SamplingOptions::default(),
            &Paint::default(),
        );
    }
});
