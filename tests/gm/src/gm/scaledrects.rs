// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/scaledrects.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect};

// From crbug.com/1442854. Draws two rects (equivalent in device space) but which vary wildly
// in their sizes and scales. In both cases the clip is what is actually determining the
// final drawn geometry. For the red rectangle case the inverted skews were becoming very small and
// ran afoul of some logic in the DMSAA code that zeroed them out.
// Port of: gm/scaledrects.cpp#L23-L61 (chrome/m156)
struct ScaledRectsGM;

impl GM for ScaledRectsGM {
    fn name(&self) -> String {
        "scaledrects".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(128, 64)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clip_rect(Rect::from_xywh(10.0, 50.0, 100.0, 10.0), None, None);

        {
            let mut blue = Paint::default();
            blue.set_color(Color::BLUE);

            canvas.set_matrix(&M44::from(Matrix::new_all(
                3.0, -0.5, 0.0, -0.5, -3.0, 0.0, 0.0, 0.0, 1.0,
            )));

            canvas.draw_rect(Rect::from_xywh(-1000.0, -1000.0, 2000.0, 2000.0), &blue);
        }

        {
            let mut red = Paint::default();
            red.set_color(Color::RED);
            red.set_blend_mode(BlendMode::Plus);

            canvas.set_matrix(&M44::from(Matrix::new_all(
                3000.0, -500.0, 0.0, -500.0, -3000.0, 0.0, 0.0, 0.0, 1.0,
            )));

            canvas.draw_rect(Rect::from_xywh(-1.0, -1.0, 2.0, 2.0), &red);
        }
    }
}

// Port of: gm/scaledrects.cpp#L65 (chrome/m156)
crate::def_gm!(ScaledRectsGM_ = "ScaledRectsGM", ScaledRectsGM);

// Port of: gm/scaledrects.cpp#L67-L82 (chrome/m156)
crate::def_simple_gm!(cliplargerect, canvas, 256, 256, {
    canvas.save();
    canvas.clip_irect(IRect::new(0, 0, 120, 256), None);
    canvas.save();
    canvas.translate((1e24, 0.0));
    canvas.clear(Color::GREEN);
    canvas.restore();
    canvas.restore();

    let mut line = Paint::default();
    line.set_style(Style::Stroke);
    line.set_color(Color::BLACK);
    canvas.draw_line((120.0, 0.0), (120.0, 256.0), &line);
});
