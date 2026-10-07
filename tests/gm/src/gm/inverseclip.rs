// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/inverseclip.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;

// Repro case for skbug.com/40040760
// Port of: gm/inverseclip.cpp#L19-L35 (chrome/m156)
crate::def_simple_gm!(inverseclip, canvas, 400, 400, {
    let mut clip = PathBuilder::new();
    clip.set_fill_type(PathFillType::InverseWinding);
    clip.move_to((195.448, 31.0));
    clip.cubic_to((97.9925, 31.0), (18.99, 105.23), (18.99, 196.797));
    clip.cubic_to((18.99, 288.365), (97.9925, 362.595), (195.448, 362.595));
    clip.cubic_to((292.905, 362.595), (371.905, 288.365), (371.905, 196.797));
    clip.cubic_to((371.905, 105.23), (292.905, 31.0), (195.448, 31.0));
    clip.close();
    canvas.clip_path(&clip.detach(), None, true);

    let mut paint = Paint::default();
    paint.set_color(Color::BLUE);
    canvas.draw_rect(Rect::from_wh(400.0, 400.0), &paint);
});
