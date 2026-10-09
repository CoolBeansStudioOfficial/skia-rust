// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pathreverse.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv::reverse_add_path;
use skia_rust_core::rect::Rect;

// The hiragino_maru_goth_pro_e path was generated with Mac-specific code.
// Port of: gm/pathreverse.cpp#L17-L43 (chrome/m156), hiragino_maru_goth_pro_e
fn hiragino_maru_goth_pro_e() -> Path {
    let mut path = PathBuilder::new();
    path.move_to((98.6, 24.7));
    path.cubic_to((101.7, 24.7), (103.6, 22.8), (103.6, 19.2));
    path.cubic_to((103.6, 18.9), (103.6, 18.7), (103.6, 18.4));
    path.cubic_to((102.6, 5.3), (94.4, -6.1), (79.8, -6.1));
    path.cubic_to((63.5, -6.1), (54.5, 6.0), (54.5, 23.3));
    path.cubic_to((54.5, 40.6), (64.0, 52.2), (80.4, 52.2));
    path.cubic_to((93.4, 52.2), (99.2, 45.6), (102.4, 39.0));
    path.cubic_to((102.8, 38.4), (102.9, 37.8), (102.9, 37.2));
    path.cubic_to((102.9, 35.4), (101.5, 34.2), (99.8, 33.7));
    path.cubic_to((99.1, 33.5), (98.4, 33.3), (97.7, 33.3));
    path.cubic_to((96.3, 33.3), (95.0, 34.0), (94.1, 35.8));
    path.cubic_to((91.7, 41.1), (87.7, 44.7), (80.5, 44.7));
    path.cubic_to((69.7, 44.7), (63.6, 37.0), (63.4, 24.7));
    path.line_to((98.6, 24.7));
    path.close();
    path.move_to((63.7, 17.4));
    path.cubic_to((65.0, 7.6), (70.2, 1.2), (79.8, 1.2));
    path.cubic_to((89.0, 1.2), (93.3, 8.5), (94.5, 15.6));
    path.cubic_to((94.5, 15.8), (94.5, 16.0), (94.5, 16.1));
    path.cubic_to((94.5, 17.0), (94.1, 17.4), (93.0, 17.4));
    path.line_to((63.7, 17.4));
    path.close();
    path.detach()
}

// Port of: gm/pathreverse.cpp#L45-L52 (chrome/m156), test_path
fn test_path(canvas: &Canvas, path: &Path) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_path(path, &paint);
    paint.set_style(Style::Stroke);
    paint.set_color(Color::RED);
    canvas.draw_path(path, &paint);
}

// Port of: gm/pathreverse.cpp#L54-L62 (chrome/m156), test_rev
fn test_rev(canvas: &Canvas, path: &Path) {
    test_path(canvas, path);
    let mut rev = PathBuilder::new();
    reverse_add_path(&mut rev, path);
    canvas.save();
    canvas.translate((150.0, 0.0));
    test_path(canvas, &rev.detach());
    canvas.restore();
}

// Port of: gm/pathreverse.cpp#L64-L97 (chrome/m156), pathreverse
crate::def_simple_gm_bg_name!(
    pathreverse,
    canvas,
    640,
    480,
    Color::WHITE,
    "path-reverse",
    {
        let mut r = Rect::from_ltrb(10.0, 10.0, 100.0, 60.0);
        let mut builder = PathBuilder::new();
        builder.add_rect(r, None, None);
        test_rev(canvas, &builder.snapshot());
        canvas.translate((0.0, 100.0));
        builder.offset((20.0, 20.0));
        builder.add_rect(r, None, None);
        test_rev(canvas, &builder.detach());
        canvas.translate((0.0, 100.0));
        builder.move_to((10.0, 10.0));
        builder.line_to((30.0, 30.0));
        builder.add_oval(r, None, None);
        r.offset((50.0, 20.0));
        builder.add_oval(r, None, None);
        test_rev(canvas, &builder.detach());
        let path = hiragino_maru_goth_pro_e();
        canvas.translate((0.0, 100.0));
        test_rev(canvas, &path);
    }
);
