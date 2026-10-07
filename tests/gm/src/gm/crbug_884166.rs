// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_884166.cpp (chrome/m156)

use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// Port of: gm/crbug_884166.cpp#L12-L29 (chrome/m156)
crate::def_simple_gm!(crbug_884166, canvas, 300, 300, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);
    let mut path = PathBuilder::new();
    path.move_to((153.25, 280.75));
    path.line_to((161.75, 281.75));
    path.line_to((164.25, 282.00));
    path.line_to((0.00, 276.00));
    path.line_to((161.50, 0.00));
    path.line_to((286.25, 231.25));
    path.line_to((163.75, 282.00));
    path.line_to((150.00, 280.00));
    canvas.draw_path(&path.detach(), &paint);
});
