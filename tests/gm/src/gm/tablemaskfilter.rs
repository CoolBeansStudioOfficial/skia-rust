// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tablemaskfilter.cpp (chrome/m156)

use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::rect::Rect;
use skia_rust_effects::table_mask_filter;

use crate::prelude::Color;

// Port of: gm/tablemaskfilter.cpp#L18-L38 (chrome/m156)
crate::def_simple_gm!(tablemaskfilter, canvas, 400, 400, {
    let mut paint = Paint::default();
    paint.set_style(Style::Fill);
    paint.set_color(Color::BLACK);

    // Add a table mask filter that defines half coverage for all coverage values
    // except pixels with full coverage.
    let mut table = [0u8; 256];
    table.fill(128);
    table[255] = 255;
    paint.set_mask_filter(table_mask_filter::new(&table));

    let path = PathBuilder::new()
        .add_rect(Rect::new(38.0, 38.0, 218.0, 218.0), None, None)
        .add_oval(
            Rect::new(38.0, 38.0, 218.0, 218.0),
            PathDirection::CCW,
            None,
        )
        .detach();

    canvas.draw_path(&path, &paint);
});
