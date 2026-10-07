// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/smallarc.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// this draws a small arc scaled up
// see https://code.google.com/p/chromium/issues/detail?id=102411
// and https://code.google.com/p/skia/issues/detail?id=2769
// Port of: gm/smallarc.cpp#L15-L33 (chrome/m156)
crate::def_simple_gm!(smallarc, canvas, 762, 762, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(120.0);

    let path = PathBuilder::new()
        .move_to((75.0, 0.0))
        .cubic_to((33.5, 0.0), (0.0, 33.5), (0.0, 75.0))
        .detach();

    canvas.translate((-400.0, -400.0));
    canvas.scale((8.0, 8.0));
    canvas.draw_path(&path, &p);
});
