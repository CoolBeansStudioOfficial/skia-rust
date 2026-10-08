// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/radial_gradient_precision.cpp (chrome/m156)

use skia_rust_core::color::colors;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// All we're looking for here is that we see a smooth gradient.
// Port of: gm/radial_gradient_precision.cpp#L19-L28 (chrome/m156)
crate::def_simple_gm!(radial_gradient_precision, canvas, 200, 200, {
    let center = Point::new(1000.0, 1000.0);
    let radius = 40.0;
    let colors = [colors::BLACK, colors::GREEN];

    let mut p = Paint::default();
    p.set_shader(shaders::radial_gradient(
        (center, radius),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Repeat, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_paint(&p);
});
