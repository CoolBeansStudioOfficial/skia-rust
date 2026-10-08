// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/repeated_bitmap.cpp (chrome/m156)
//
// `repeated_bitmap_jpg` (color_wheel.jpg) is not ported: JPEG decoding is not ported yet.

use skia_rust_core::image::Image;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;

use crate::prelude::*;
use crate::tool_utils::{draw_checkerboard, get_resource_as_image, int_to_scalar};

// Port of: gm/repeated_bitmap.cpp#L22-L46 (chrome/m156)
fn draw_rotated_image(
    canvas: &Canvas,
    image: Option<&Image>,
    error_msg: &mut String,
) -> DrawResult {
    // SkColorSetRGB(156, 154, 156)
    draw_checkerboard(
        canvas,
        Color::from_argb(0xFF, 156, 154, 156),
        Color::WHITE,
        12,
    );
    let Some(image) = image else {
        *error_msg = "No image. Did you forget to set the resourcePath?".to_string();
        return DrawResult::Fail;
    };
    let rect = Rect::from_ltrb(-68.0, -68.0, 68.0, 68.0);
    let mut paint = Paint::default();
    // SkColorSetRGB(49, 48, 49)
    paint.set_color(Color::from_argb(0xFF, 49, 48, 49));
    let scale =
        (128.0_f32 / int_to_scalar(image.width())).min(128.0_f32 / int_to_scalar(image.height()));
    let origin = [
        -0.5_f32 * int_to_scalar(image.width()),
        -0.5_f32 * int_to_scalar(image.height()),
    ];
    for j in 0..4 {
        for i in 0..4 {
            let saved = canvas.save();
            canvas.translate((
                96.0 + 192.0 * int_to_scalar(i),
                96.0 + 192.0 * int_to_scalar(j),
            ));
            canvas.rotate(18.0 * int_to_scalar(i + 4 * j), None);
            canvas.draw_rect(rect, &paint);
            canvas.scale((scale, scale));
            canvas.draw_image(image, (origin[0], origin[1]), None);
            canvas.restore_to_count(saved);
        }
    }
    DrawResult::Ok
}

// Port of: gm/repeated_bitmap.cpp#L48-L51 (chrome/m156)
crate::def_simple_gm_can_fail!(repeated_bitmap, canvas, error_msg, 576, 576, {
    let image = get_resource_as_image("images/randPixels.png");
    draw_rotated_image(canvas, image.as_ref(), error_msg)
});
