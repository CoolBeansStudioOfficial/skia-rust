// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1041204.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;

// Port of: gm/crbug_1041204.cpp#L21-L24 (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // the C++ literals, digit for digit
fn matrix() -> Matrix {
    Matrix::new_all(
        -0.0005550860255665798,
        -0.0030798374421905717,
        -0.014111959825129805,
        -0.07569627776417084,
        232.00000000000017,
        39.999999999999936,
        0.0,
        0.0,
        1.0,
    )
}

// Port of: gm/crbug_1041204.cpp#L16-L34 (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // the C++ literals, digit for digit
fn draw(canvas: &Canvas) {
    // While the coordinates are giant and the transform is not axis-aligned, this should
    // fill the screen left side with solid blue. This has an extra zoom factor compared to the
    // canvas JS in order to more visibly highlight the numerical issues that caused the bug.
    // (The original transform would have completely filled the screen with solid blue, so the bug
    // manifested as an improper discard on occasion. With the new scale factor, the bug manifests
    // as either an improper fullscreen clear or an improper discard, instead).
    let extra_zoom: f32 = (-2.3f32).exp();
    canvas.scale((extra_zoom, extra_zoom));
    canvas.scale((2.0, 2.0));
    canvas.concat(&matrix());
    canvas.translate((-3040103.0493857153, 337502.1103282161));
    canvas.scale((9783.93962050256, -9783.93962050256));
    let mut paint = Paint::default();
    paint.set_color(Color::BLUE);
    paint.set_anti_alias(true);
    canvas.draw_rect(Rect::from_wh(512.0, 512.0), &paint);
}

crate::def_simple_gm!(crbug_10141204, canvas, 512, 512, { draw(canvas) });
