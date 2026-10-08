// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tilemodes_alpha.cpp (chrome/m156)

use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;

use crate::prelude::*;
use crate::tool_utils::{get_resource_as_image, int_to_scalar};

// Port of: gm/tilemodes_alpha.cpp#L19-L39 (chrome/m156)
crate::def_simple_gm!(tilemodes_alpha, canvas, 512, 512, {
    let Some(image) = get_resource_as_image("images/mandrill_64.png") else {
        return;
    };
    let modes = [
        TileMode::Clamp,
        TileMode::Repeat,
        TileMode::Mirror,
        TileMode::Decal,
    ];
    for y in 0..4 {
        for x in 0..4 {
            let rect = Rect::from_xywh(
                int_to_scalar(128 * x + 1),
                int_to_scalar(128 * y + 1),
                126.0,
                126.0,
            );
            let matrix = Matrix::translate((rect.x(), rect.y()));
            let mut paint = Paint::default();
            paint.set_color4f(Color4f::new(0.0, 0.0, 0.0, 0.5), None);
            paint.set_shader(image.to_shader(
                (
                    modes[usize::try_from(x).expect("x >= 0")],
                    modes[usize::try_from(y).expect("y >= 0")],
                ),
                SamplingOptions::default(),
                &matrix,
            ));
            canvas.draw_rect(rect, &paint);
        }
    }
});
