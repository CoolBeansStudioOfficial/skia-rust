// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_908646.cpp (chrome/m156)

use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;

// Port of: gm/crbug_908646.cpp#L12-L29 (chrome/m156)
crate::def_simple_gm!(crbug_908646, canvas, 300, 300, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((50.0, 50.0));
    path.line_to((50.0, 300.0));
    path.line_to((250.0, 300.0));
    path.line_to((250.0, 50.0));
    path.move_to((200.0, 100.0));
    path.line_to((100.0, 100.0));
    path.line_to((150.0, 200.0));
    path.move_to((100.0, 250.0));
    path.line_to((150.0, 150.0));
    path.line_to((200.0, 250.0));
    canvas.draw_path(&path.detach(), &paint);
});
