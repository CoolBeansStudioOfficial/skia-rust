// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fillrect_gradient.cpp (chrome/m156)

// This GM creates the same gradients as the Chromium test fillrect_gradient:
// http://osscs/chromium/chromium/src/+/main:third_party/blink/web_tests/fast/canvas/fillrect_gradient.html

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/fillrect_gradient.cpp#L31-L34 (chrome/m156)
const K_CELL_SIZE: i32 = 50;
const K_NUM_COLUMNS: i32 = 2;
const K_NUM_ROWS: i32 = 9;
const K_PAD_SIZE: i32 = 10;

// Port of: gm/fillrect_gradient.cpp#L36-L132 (chrome/m156)
struct FillrectGradientGm;

// Port of: gm/fillrect_gradient.cpp#L43-L46 (chrome/m156)
struct GradientStop {
    pos: f32,
    color: Color,
}

impl FillrectGradientGm {
    // Port of: gm/fillrect_gradient.cpp#L55-L92 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // SkIntToScalar of small values
    fn draw_gradient(canvas: &Canvas, stops: &[GradientStop]) {
        let mut colors: Vec<Color4f> = Vec::with_capacity(stops.len());
        let mut positions: Vec<f32> = Vec::with_capacity(stops.len());

        for stop in stops {
            colors.push(Color4f::from_color(stop.color));
            positions.push(stop.pos);
        }

        let k = K_CELL_SIZE as f32;
        let points = [Point::new(k, 0.0), Point::new(k, k)];

        // Draw the gradient linearly.
        let shader = shaders::linear_gradient(
            (points[0], points[1]),
            &Gradient::new(
                Colors::new(&colors, Some(&positions), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
        let mut paint = Paint::default();
        paint.set_shader(shader);
        canvas.draw_rect(Rect::from_xywh(0.0, 0.0, k, k), &paint);

        canvas.save();
        canvas.translate(((K_CELL_SIZE + K_PAD_SIZE) as f32, 0.0));

        // Draw the gradient radially.
        let shader = shaders::radial_gradient(
            (
                Point::new((K_CELL_SIZE / 2) as f32, (K_CELL_SIZE / 2) as f32),
                (K_CELL_SIZE / 2) as f32,
            ),
            &Gradient::new(
                Colors::new(&colors, Some(&positions), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
        paint.set_shader(shader);
        canvas.draw_rect(Rect::from_xywh(0.0, 0.0, k, k), &paint);

        canvas.restore();
        canvas.translate((0.0, (K_CELL_SIZE + K_PAD_SIZE) as f32));
    }
}

impl GM for FillrectGradientGm {
    fn name(&self) -> String {
        "fillrect_gradient".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(
            K_NUM_COLUMNS * (K_CELL_SIZE + K_PAD_SIZE),
            K_NUM_ROWS * (K_CELL_SIZE + K_PAD_SIZE),
        )
    }

    // Port of: gm/fillrect_gradient.cpp#L94-L128 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let s = |pos: f32, color: Color| GradientStop { pos, color };

        // Simple gradient: Green to white
        Self::draw_gradient(canvas, &[s(0.0, Color::GREEN), s(1.0, Color::WHITE)]);

        // Multiple sections: Green to white to red
        Self::draw_gradient(
            canvas,
            &[
                s(0.0, Color::GREEN),
                s(0.5, Color::WHITE),
                s(1.0, Color::RED),
            ],
        );

        // No stops at 0.0 or 1.0: Larger green to white to larger red
        Self::draw_gradient(
            canvas,
            &[
                s(0.4, Color::GREEN),
                s(0.5, Color::WHITE),
                s(0.6, Color::RED),
            ],
        );

        // Only one stop, at zero: Solid red
        Self::draw_gradient(canvas, &[s(0.0, Color::RED)]);

        // Only one stop, at 1.0: Solid red
        Self::draw_gradient(canvas, &[s(1.0, Color::RED)]);

        // Only one stop, in the middle: Solid red
        Self::draw_gradient(canvas, &[s(0.5, Color::RED)]);

        // Disjoint gradients (multiple stops at the same offset)
        // Blue to white in the top (inner) half, red to yellow in the bottom (outer) half
        Self::draw_gradient(
            canvas,
            &[
                s(0.0, Color::BLUE),
                s(0.5, Color::WHITE),
                s(0.5, Color::RED),
                s(1.0, Color::YELLOW),
            ],
        );

        // Ignored stops: Blue to white, red to yellow (same as previous)
        Self::draw_gradient(
            canvas,
            &[
                s(0.0, Color::BLUE),
                s(0.5, Color::WHITE),
                s(0.5, Color::GRAY),
                s(0.5, Color::CYAN),
                s(0.5, Color::RED),
                s(1.0, Color::YELLOW),
            ],
        );

        // Unsorted stops: Blue to white, red to yellow
        // Unlike Chrome, we don't sort the stops, so this renders differently than the prior
        // cell.
        Self::draw_gradient(
            canvas,
            &[
                s(0.5, Color::WHITE),
                s(0.5, Color::GRAY),
                s(1.0, Color::YELLOW),
                s(0.5, Color::CYAN),
                s(0.5, Color::RED),
                s(0.0, Color::BLUE),
            ],
        );
    }
}

// Port of: gm/fillrect_gradient.cpp#L141 (chrome/m156)
crate::def_gm!(FillrectGradientGM, FillrectGradientGm);
