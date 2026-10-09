// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/orientation.cpp (chrome/m156), the draw functions and the six JPEG subsampling
// variants. `make_images` (which writes the PNG sources, `if (false)` in Skia) is not ported.

use skia_rust_codec::image_generator_from_encoded::make_from_encoded;
use skia_rust_core::data::Data;
use skia_rust_core::images;

use crate::SimpleGM;
use crate::prelude::*;
use crate::tool_utils::{get_resource_as_data, get_resource_as_image, int_to_scalar};

// Port of: gm/orientation.cpp#L19-L20 (kImgW, kImgH; only the GM sizes use them)
const IMG_W: i32 = 100;
const IMG_H: i32 = 80;

// Port of: gm/orientation.cpp#L131-L150 (draw)
// Draws the eight oriented images in two rows of four, each row break after image 4.
fn draw(canvas: &Canvas, suffix: &str) {
    canvas.save();
    for i in '1'..='8' {
        let path = format!("images/orientation/{i}{suffix}.jpg");
        let Some(image) = get_resource_as_image(&path) else {
            continue;
        };
        canvas.draw_image(&image, (0, 0), None);
        if i == '4' {
            canvas.restore();
            canvas.translate((0.0, int_to_scalar(image.height())));
        } else {
            canvas.translate((int_to_scalar(image.width()), 0.0));
        }
    }
}

// Port of: gm/orientation.cpp#L153-L156 (MAKE_GM and the DEF_SIMPLE_GM it expands to)
fn draw_410(canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
    draw(canvas, "_410");
    DrawResult::Ok
}

fn draw_411(canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
    draw(canvas, "_411");
    DrawResult::Ok
}

fn draw_420(canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
    draw(canvas, "_420");
    DrawResult::Ok
}

fn draw_422(canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
    draw(canvas, "_422");
    DrawResult::Ok
}

fn draw_440(canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
    draw(canvas, "_440");
    DrawResult::Ok
}

fn draw_444(canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
    draw(canvas, "_444");
    DrawResult::Ok
}

// The manifest names are the subsampling numbers, which are not identifiers, so the GMs are
// registered with `def_gm!`'s explicit name.
crate::def_gm!(
    orientation_410 = "410",
    SimpleGM::new(
        Color::WHITE,
        "orientation_410",
        ISize::new(4 * IMG_W, 2 * IMG_H),
        draw_410
    )
);
crate::def_gm!(
    orientation_411 = "411",
    SimpleGM::new(
        Color::WHITE,
        "orientation_411",
        ISize::new(4 * IMG_W, 2 * IMG_H),
        draw_411
    )
);
crate::def_gm!(
    orientation_420 = "420",
    SimpleGM::new(
        Color::WHITE,
        "orientation_420",
        ISize::new(4 * IMG_W, 2 * IMG_H),
        draw_420
    )
);
crate::def_gm!(
    orientation_422 = "422",
    SimpleGM::new(
        Color::WHITE,
        "orientation_422",
        ISize::new(4 * IMG_W, 2 * IMG_H),
        draw_422
    )
);
crate::def_gm!(
    orientation_440 = "440",
    SimpleGM::new(
        Color::WHITE,
        "orientation_440",
        ISize::new(4 * IMG_W, 2 * IMG_H),
        draw_440
    )
);
crate::def_gm!(
    orientation_444 = "444",
    SimpleGM::new(
        Color::WHITE,
        "orientation_444",
        ISize::new(4 * IMG_W, 2 * IMG_H),
        draw_444
    )
);

// Port of: gm/orientation.cpp#L158-L181 (respect_orientation_jpeg)
// This GM demonstrates that the default SkImageGenerator respects the orientation flag.
crate::def_simple_gm!(respect_orientation_jpeg, canvas, 4 * IMG_W, 2 * IMG_H, {
    canvas.save();
    for i in '1'..='8' {
        let path = format!("images/orientation/{i}_444.jpg");
        // Get the image as data.
        let Some(bytes) = get_resource_as_data(&path) else {
            continue;
        };
        // SkCodecImageGenerator::MakeFromEncodedCodec(data), through the encoded-data factory.
        let data = Data::new_copy(&bytes);
        let image = images::deferred_from_generator(make_from_encoded(Some(data), None));
        let Some(image) = image else {
            continue;
        };
        canvas.draw_image(&image, (0, 0), None);
        if i == '4' || i == '8' {
            canvas.restore();
            canvas.translate((0.0, int_to_scalar(image.height())));
        } else {
            canvas.translate((int_to_scalar(image.width()), 0.0));
        }
    }
});
