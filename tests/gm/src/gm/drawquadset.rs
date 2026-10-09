// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawquadset.cpp (chrome/m156)

// Casts and names mirror the C++ arithmetic and loop counters this file ports, so they stay
// as written in the source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names
)]

use crate::prelude::*;
use skia_rust_core::canvas::QuadAAFlags;
use skia_rust_core::color::Color4f;
use skia_rust_core::font::Font;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const TILE_WIDTH: f32 = 40.0;
const TILE_HEIGHT: f32 = 30.0;
const ROW_COUNT: i32 = 4;
const COL_COUNT: i32 = 3;

// Port of: gm/drawquadset.cpp#L44-L47 (chrome/m156)
fn draw_text(canvas: &Canvas, text: &str) {
    let font = Font::from_size(default_portable_typeface(), 12.0);
    canvas.draw_str(text, (0.0, 0.0), &font, &Paint::default());
}

// Port of: gm/drawquadset.cpp#L49-L108 (chrome/m156)
fn draw_gradient_tiles(canvas: &Canvas, align_gradients: bool) {
    // The gradient only feeds the GPU-only fillRectWithEdgeAA branch of the C++ (it needs a
    // SurfaceDrawContext, which is null on the raster backend), so it is not built here.
    for i in 0..ROW_COUNT {
        for j in 0..COL_COUNT {
            let mut tile = Rect::from_wh(TILE_WIDTH, TILE_HEIGHT);
            if align_gradients {
                tile.offset((j as f32 * TILE_WIDTH, i as f32 * TILE_HEIGHT));
            } else {
                canvas.save();
                canvas.translate((j as f32 * TILE_WIDTH, i as f32 * TILE_HEIGHT));
            }
            let aa = edge_flags(i, j);
            // Fallback to solid color on raster backend since the public API only has color
            let color = if align_gradients || (i * COL_COUNT + j) % 2 == 0 {
                Color::BLUE
            } else {
                Color::WHITE
            };
            canvas.experimental_draw_edge_aa_quad(tile, None, aa, color, BlendMode::SrcOver);
            if !align_gradients {
                // Pop off the matrix translation when drawing unaligned
                canvas.restore();
            }
        }
    }
}

/// The edges of tile `(i, j)` on the outside of the grid (`aa` in both C++ tile functions).
fn edge_flags(i: i32, j: i32) -> QuadAAFlags {
    let mut aa = QuadAAFlags::NONE;
    if i == 0 {
        aa |= QuadAAFlags::TOP;
    }
    if i == ROW_COUNT - 1 {
        aa |= QuadAAFlags::BOTTOM;
    }
    if j == 0 {
        aa |= QuadAAFlags::LEFT;
    }
    if j == COL_COUNT - 1 {
        aa |= QuadAAFlags::RIGHT;
    }
    aa
}

// Port of: gm/drawquadset.cpp#L110-L141 (chrome/m156)
fn draw_color_tiles(canvas: &Canvas, multicolor: bool) {
    for i in 0..ROW_COUNT {
        for j in 0..COL_COUNT {
            let tile = Rect::from_xywh(
                j as f32 * TILE_WIDTH,
                i as f32 * TILE_HEIGHT,
                TILE_WIDTH,
                TILE_HEIGHT,
            );
            let color = if multicolor {
                Color4f::new(
                    (i as f32 + 1.0) / ROW_COUNT as f32,
                    (j as f32 + 1.0) / COL_COUNT as f32,
                    0.4,
                    1.0,
                )
            } else {
                Color4f::new(0.2, 0.8, 0.3, 1.0)
            };
            canvas.experimental_draw_edge_aa_quad(
                tile,
                None,
                edge_flags(i, j),
                color.to_color(),
                BlendMode::SrcOver,
            );
        }
    }
}

// Port of: gm/drawquadset.cpp#L143-L167 (chrome/m156)
fn draw_tile_boundaries(canvas: &Canvas, local: &Matrix) {
    // Draw grid of red lines at interior tile boundaries.
    const LINE_OUTSET: f32 = 10.0;
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::RED);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);
    for x in 1..COL_COUNT {
        let mut pts = [
            Point::from((x as f32 * TILE_WIDTH, 0.0)),
            Point::from((x as f32 * TILE_WIDTH, ROW_COUNT as f32 * TILE_HEIGHT)),
        ];
        local.map_points_inplace(&mut pts);
        let mut v = pts[1] - pts[0];
        v.set_length(v.length() + LINE_OUTSET);
        canvas.draw_line(pts[1] - v, pts[0] + v, &paint);
    }
    for y in 1..ROW_COUNT {
        let mut pts = [
            Point::from((0.0, y as f32 * TILE_HEIGHT)),
            Point::from((TILE_WIDTH * COL_COUNT as f32, y as f32 * TILE_HEIGHT)),
        ];
        local.map_points_inplace(&mut pts);
        let mut v = pts[1] - pts[0];
        v.set_length(v.length() + LINE_OUTSET);
        canvas.draw_line(pts[1] - v, pts[0] + v, &paint);
    }
}

/// The tile renderers, one per column (`kTileSets`), in `kTileSetNames` order.
// Port of: gm/drawquadset.cpp#L169-L183 (chrome/m156)
fn draw_tile_set(canvas: &Canvas, index: usize) {
    match index {
        0 => draw_gradient_tiles(canvas, false),
        1 => draw_gradient_tiles(canvas, true),
        2 => draw_color_tiles(canvas, false),
        _ => draw_color_tiles(canvas, true),
    }
}

const TILE_SET_NAMES: [&str; 4] = ["Local", "Aligned", "Green", "Multicolor"];
const MATRIX_NAMES: [&str; 5] = ["Identity", "T+S", "Rotate", "Skew", "Perspective"];

#[derive(Default)]
struct DrawQuadSetGm;

impl GM for DrawQuadSetGm {
    fn name(&self) -> String {
        "draw_quad_set".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 800)
    }

    // Port of: gm/drawquadset.cpp#L190-L255 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        // SkMatrix's default constructor is the identity.
        let mut row_matrices = [(); 5].map(|()| Matrix::new_identity());
        // Identity
        row_matrices[0].set_identity();
        // Translate/scale
        row_matrices[1].set_translate((5.5, 20.25));
        row_matrices[1].post_scale((0.9, 0.7), None);
        // Rotation
        row_matrices[2].set_rotate(20.0, None);
        row_matrices[2].pre_translate((15.0, -20.0));
        // Skew
        row_matrices[3].set_skew((0.5, 0.25), None);
        row_matrices[3].pre_translate((-30.0, 0.0));
        // Perspective
        let src = Rect::from_wh(
            COL_COUNT as f32 * TILE_WIDTH,
            ROW_COUNT as f32 * TILE_HEIGHT,
        )
        .to_quad(None::<PathDirection>);
        let dst = [
            Point::from((0.0, 0.0)),
            Point::from((COL_COUNT as f32 * TILE_WIDTH + 10.0, 15.0)),
            Point::from((
                COL_COUNT as f32 * TILE_WIDTH - 28.0,
                ROW_COUNT as f32 * TILE_HEIGHT + 40.0,
            )),
            Point::from((25.0, ROW_COUNT as f32 * TILE_HEIGHT - 15.0)),
        ];
        assert!(row_matrices[4].set_poly_to_poly(&src, &dst));
        row_matrices[4].pre_translate((0.0, 10.0));

        // Print a column header
        canvas.save();
        canvas.translate((110.0, 20.0));
        for name in TILE_SET_NAMES {
            draw_text(canvas, name);
            canvas.translate((COL_COUNT as f32 * TILE_WIDTH + 30.0, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, 40.0));

        // Render all tile variations
        for (i, matrix) in row_matrices.iter().enumerate() {
            canvas.save();
            canvas.translate((10.0, 0.5 * ROW_COUNT as f32 * TILE_HEIGHT));
            draw_text(canvas, MATRIX_NAMES[i]);
            canvas.translate((100.0, -0.5 * ROW_COUNT as f32 * TILE_HEIGHT));
            for j in 0..TILE_SET_NAMES.len() {
                canvas.save();
                draw_tile_boundaries(canvas, matrix);
                canvas.concat(matrix);
                draw_tile_set(canvas, j);
                // Undo the local transformation
                canvas.restore();
                // And advance to the next column
                canvas.translate((COL_COUNT as f32 * TILE_WIDTH + 30.0, 0.0));
            }
            // Reset back to the left edge
            canvas.restore();
            // And advance to the next row
            canvas.translate((0.0, ROW_COUNT as f32 * TILE_HEIGHT + 20.0));
        }
    }
}

// Port of: gm/drawquadset.cpp#L257-L257 (chrome/m156)
crate::def_gm!(DrawQuadSetGm_ = "DrawQuadSetGM()", DrawQuadSetGm);
