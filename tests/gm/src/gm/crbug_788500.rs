// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_788500.cpp (chrome/m156)

use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;

// Port of: gm/crbug_788500.cpp#L12-L23 (chrome/m156)
crate::def_simple_gm!(crbug_788500, canvas, 300, 300, {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.move_to((245.5, 98.5));
    path.cubic_to((245.5, 98.5), (242.0, 78.0), (260.0, 75.0));
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_path(&path.detach(), &paint);
});
