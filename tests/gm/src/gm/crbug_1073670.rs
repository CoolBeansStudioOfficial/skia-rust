// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1073670.cpp (chrome/m156)

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
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/crbug_1073670.cpp#L9-L22 (chrome/m156)
crate::def_simple_gm!(crbug_1073670, canvas, 250, 250, {
    let pts = [Point::new(0.0, 0.0), Point::new(0.0, 250.0)];
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
    ];
    let sh = shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    );
    let mut p = Paint::default();
    p.set_shader(sh);
    let mut f: Font = default_portable_font();
    f.set_size(325.0);
    f.set_edging(Edging::AntiAlias);
    draw_string(canvas, "Gradient", 10.0, 250.0, &f, &p, Align::Left);
});
