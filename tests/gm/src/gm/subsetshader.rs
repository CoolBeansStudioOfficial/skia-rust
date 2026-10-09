// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/subsetshader.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::get_resource_as_bitmap;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/subsetshader.cpp#L13-L41 (chrome/m156), bitmap_subset_shader
crate::def_simple_gm_can_fail!(bitmap_subset_shader, canvas, error_msg, 256, 256, {
    canvas.clear(Color::WHITE);
    let Some(source) = get_resource_as_bitmap("images/color_wheel.png") else {
        error_msg.clear();
        error_msg.push_str(
            "Could not load images/color_wheel.png. Did you forget to set the resourcePath?",
        );
        return DrawResult::Fail;
    };
    let left = IRect::from_wh(source.width() / 2, source.height());
    let right = IRect::from_xywh(source.width() / 2, 0, source.width() / 2, source.height());
    let mut left_bitmap = Bitmap::new();
    let mut right_bitmap = Bitmap::new();
    let _ = source.extract_subset(&mut left_bitmap, left);
    let _ = source.extract_subset(&mut right_bitmap, right);
    let mut matrix = Matrix::new_identity();
    matrix.set_scale((0.75, 0.75), None);
    matrix.pre_rotate(30.0, None);
    let tm = TileMode::Repeat;
    let mut paint = Paint::default();
    paint.set_shader(left_bitmap.to_shader((tm, tm), SamplingOptions::default(), &matrix));
    canvas.draw_rect(Rect::new(0.0, 0.0, 256.0, 128.0), &paint);
    paint.set_shader(right_bitmap.to_shader((tm, tm), SamplingOptions::default(), &matrix));
    canvas.draw_rect(Rect::new(0.0, 128.0, 256.0, 256.0), &paint);
    DrawResult::Ok
});
