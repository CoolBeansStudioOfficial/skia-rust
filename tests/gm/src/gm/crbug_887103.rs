// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_887103.cpp (chrome/m156)

use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// Port of: gm/crbug_887103.cpp#L12-L32 (chrome/m156)
crate::def_simple_gm!(crbug_887103, canvas, 520, 520, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);
    let mut path = PathBuilder::new();
    path.move_to((510.0, 20.0));
    path.line_to((500.0, 20.0));
    path.line_to((510.0, 500.0));
    path.move_to((500.0, 20.0));
    path.line_to((510.0, 500.0));
    path.line_to((500.0, 510.0));
    path.move_to((500.0, 30.0));
    path.line_to((510.0, 10.0));
    path.line_to((10.0, 30.0));
    canvas.draw_path(&path.detach(), &paint);
});
