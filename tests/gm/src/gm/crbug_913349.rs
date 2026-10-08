// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_913349.cpp (chrome/m156)

use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// Port of: gm/crbug_913349.cpp#L12-L28 (chrome/m156)
crate::def_simple_gm!(crbug_913349, canvas, 500, 600, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);
    // This is a reduction from crbug.com/913349 to 5 verts.
    let mut path = PathBuilder::new();
    path.move_to((349.5, 225.75));
    path.line_to((96.5, 74.0));
    path.line_to((500.50, 226.0));
    path.line_to((350.0, 226.0));
    path.line_to((350.0, 224.0));
    canvas.draw_path(&path.detach(), &paint);
});
