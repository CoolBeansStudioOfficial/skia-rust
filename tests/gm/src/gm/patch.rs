// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/patch.cpp (chrome/m156)
//
// Not ported yet (manifest stays `todo`): `patch_primitive`, `patch_image`, `patch_image_persp`
// and `patch_alpha` all go through `dopatch`, which draws with `make_shader()` (an
// `SkShaders::LinearGradient`, gradients are Phase 3) or an image shader
// (`SkImage::makeShader`, `ToolUtils::GetResourceAsImage`).

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::utils::patch_utils;

use crate::prelude::*;

// The order of the colors and points is clockwise starting at upper-left corner.
// Port of: gm/patch.cpp#L81-L92 (chrome/m156)
const G_CUBICS: [Point; patch_utils::NUM_CTRL_PTS] = [
    // top points
    Point::new(100.0, 100.0),
    Point::new(150.0, 50.0),
    Point::new(250.0, 150.0),
    Point::new(300.0, 100.0),
    // right points
    Point::new(250.0, 150.0),
    Point::new(350.0, 250.0),
    // bottom points
    Point::new(300.0, 300.0),
    Point::new(250.0, 250.0),
    Point::new(150.0, 350.0),
    Point::new(100.0, 300.0),
    // left points
    Point::new(50.0, 250.0),
    Point::new(150.0, 150.0),
];

// These two should look the same (one patch, one simple path)
// Port of: gm/patch.cpp#L186-L208 (chrome/m156)
crate::def_simple_gm!(patch_alpha_test, canvas, 550, 250, {
    canvas.translate((-75.0, -75.0));

    let colors = [Color::from(0x80FF_0000); patch_utils::NUM_CORNERS];
    let mut paint = Paint::default();
    canvas.draw_patch(&G_CUBICS, &colors, None, BlendMode::Dst, &paint);

    canvas.translate((300.0, 0.0));

    let path = PathBuilder::new()
        .move_to(G_CUBICS[0])
        .cubic_to(G_CUBICS[1], G_CUBICS[2], G_CUBICS[3])
        .cubic_to(G_CUBICS[4], G_CUBICS[5], G_CUBICS[6])
        .cubic_to(G_CUBICS[7], G_CUBICS[8], G_CUBICS[9])
        .cubic_to(G_CUBICS[10], G_CUBICS[11], G_CUBICS[0])
        .detach();
    paint.set_color(colors[0]);
    canvas.draw_path(&path, &paint);
});
