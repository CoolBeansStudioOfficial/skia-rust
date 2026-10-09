// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_938592.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use skia_rust_core::color::Color4f;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/crbug_938592.cpp#L9-L36 (chrome/m156)
crate::def_simple_gm!(crbug_938592, canvas, 500, 300, {
    let pts = [Point::new(0.0, 0.0), Point::new(0.0, 30.0)];
    // `20.0 / 20.0` is kept as the C++ spells it (`20.f / 20`).
    #[allow(clippy::eq_op)]
    let pos: [f32; 6] = [
        0.0,
        9.0 / 20.0,
        9.0 / 20.0,
        11.0 / 20.0,
        11.0 / 20.0,
        20.0 / 20.0,
    ];
    let c0 = Color4f::new(0.0, 0.0, 1.0, 1.0);
    let c1 = Color4f::new(1.0, 0.0, 0.0, 1.0);
    let c2 = Color4f::new(0.0, 1.0, 0.0, 1.0);
    let colors = [c0, c0, c1, c1, c2, c2];
    let grad = shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    );
    let mut paint = Paint::default();
    paint.set_shader(grad);

    const K_MIRROR_X: i32 = 400;
    const K_MIRROR_Y: i32 = 200;
    canvas.translate((50.0, 50.0));
    for i in 0..4 {
        canvas.save();
        if i & 0b01 != 0 {
            canvas.translate((0.0, K_MIRROR_Y as f32));
            canvas.scale((1.0, -1.0));
        }
        if i & 0b10 != 0 {
            canvas.translate((K_MIRROR_X as f32, 0.0));
            canvas.scale((-1.0, 1.0));
        }
        canvas.draw_rect(Rect::from_wh(150.0, 30.0), &paint);
        canvas.restore();
    }
});
