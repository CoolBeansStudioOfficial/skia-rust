// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/largeclippedpath.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_PI;

const K_SIZE: i32 = 1000;

// Makes sure PathInnerTriangulateOp uses correct stencil settings when there is a clip in the
// stencil buffer.
// Port of: gm/largeclippedpath.cpp#L19-L47 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar and int -> float arithmetic as in C++
fn draw_clipped_flower(canvas: &Canvas, fill_type: PathFillType) {
    canvas.clear(Color::CYAN);
    let mut clip = PathBuilder::new();
    clip.set_fill_type(PathFillType::Winding);
    const K_GRID_COUNT: i32 = 50;
    let k_cell_size: f32 = K_SIZE as f32 / K_GRID_COUNT as f32;
    let size = K_SIZE as f32;
    for y in 0..K_GRID_COUNT {
        let dir = if y & 1 == 0 {
            PathDirection::CW
        } else {
            PathDirection::CCW
        };
        clip.add_rect(
            Rect::new(0.0, y as f32 * k_cell_size, size, (y + 1) as f32 * k_cell_size),
            dir,
            None,
        );
    }
    for x in 0..K_GRID_COUNT {
        let dir = if x & 1 == 0 {
            PathDirection::CW
        } else {
            PathDirection::CCW
        };
        clip.add_rect(
            Rect::new(x as f32 * k_cell_size, 0.0, (x + 1) as f32 * k_cell_size, size),
            dir,
            None,
        );
    }
    canvas.clip_path(&clip.detach(), None, None);
    let mut flower = PathBuilder::new_with_fill_type(fill_type);
    flower.move_to((1.0, 0.0));
    const K_NUM_PETALS: i32 = 9;
    for i in 1..=K_NUM_PETALS {
        let c: f32 = 2.0 * SCALAR_PI * (i as f32 - 0.5) / K_NUM_PETALS as f32;
        let theta: f32 = 2.0 * SCALAR_PI * i as f32 / K_NUM_PETALS as f32;
        flower.quad_to((c.cos() * 2.0, c.sin() * 2.0), (theta.cos(), theta.sin()));
    }
    flower.close();
    flower.add_arc(Rect::new(-0.75, -0.75, 0.75, 0.75), 0.0, 360.0);
    canvas.translate((size / 2.0, size / 2.0));
    canvas.scale((size / 3.0, size / 3.0));
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_color(Color::MAGENTA);
    canvas.draw_path(&flower.detach(), &p);
}

// Port of: gm/largeclippedpath.cpp#L49-L51 (chrome/m156)
crate::def_simple_gm!(largeclippedpath_winding, canvas, K_SIZE, K_SIZE, {
    draw_clipped_flower(canvas, PathFillType::Winding);
});

// Port of: gm/largeclippedpath.cpp#L53-L55 (chrome/m156)
crate::def_simple_gm!(largeclippedpath_evenodd, canvas, K_SIZE, K_SIZE, {
    draw_clipped_flower(canvas, PathFillType::EvenOdd);
});
