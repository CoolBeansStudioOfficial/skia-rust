// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/circularclips.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::rect::Rect;

// Port of: gm/circularclips.cpp#L21-L84 (chrome/m156)
struct CircularClipsGM {
    x1: f32,
    x2: f32,
    y: f32,
    r: f32,
    circle1: Path,
    circle2: Path,
}

impl CircularClipsGM {
    fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y: 0.0,
            r: 0.0,
            circle1: Path::default(),
            circle2: Path::default(),
        }
    }
}

impl GM for CircularClipsGM {
    fn name(&self) -> String {
        "circular-clips".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 200)
    }

    // Port of: gm/circularclips.cpp#L26-L33 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.x1 = 80.0;
        self.x2 = 120.0;
        self.y = 50.0;
        self.r = 40.0;

        self.circle1 = Path::circle((self.x1, self.y), self.r, PathDirection::CW);
        self.circle2 = Path::circle((self.x2, self.y), self.r, PathDirection::CW);
    }

    // Port of: gm/circularclips.cpp#L43-L82 (chrome/m156)
    #[allow(clippy::manual_midpoint)] // mirrors the C++ arithmetic
    fn on_draw(&mut self, canvas: &Canvas) {
        let ops = [ClipOp::Difference, ClipOp::Intersect];

        let rect = Rect::from_ltrb(
            self.x1 - self.r,
            self.y - self.r,
            self.x2 + self.r,
            self.y + self.r,
        );

        let mut fill_paint = Paint::default();

        // Giant background circular clips (AA, non-inverted, replace/isect)
        fill_paint.set_color(Color::from(0x8080_8080));
        canvas.save();
        canvas.scale((10.0, 10.0));
        canvas.translate((
            -((self.x1 + self.x2) / 2.0 - self.r),
            -(self.y - 2.0 * self.r / 3.0),
        ));
        canvas.clip_path(&self.circle1, None, true);
        canvas.clip_path(&self.circle2, None, true);

        canvas.draw_rect(rect, &fill_paint);

        canvas.restore();

        fill_paint.set_color(Color::from(0xFF00_0000));

        for i in 0..4 {
            self.circle1.toggle_inverse_fill_type();
            if i % 2 == 0 {
                self.circle2.toggle_inverse_fill_type();
            }

            canvas.save();
            for op in ops {
                canvas.save();

                canvas.clip_path(&self.circle1, None, None);
                canvas.clip_path(&self.circle2, op, None);

                canvas.draw_rect(rect, &fill_paint);

                canvas.restore();
                canvas.translate((0.0, 2.0 * self.y));
            }
            canvas.restore();
            canvas.translate((self.x1 + self.x2, 0.0));
        }
    }
}

// Port of: gm/circularclips.cpp#L90 (chrome/m156)
crate::def_gm!(CircularClipsGM_ = "CircularClipsGM", CircularClipsGM::new());
