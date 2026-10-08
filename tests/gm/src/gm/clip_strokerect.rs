// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/clip_strokerect.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;

// Port of: gm/clip_strokerect.cpp#L16-L67 (chrome/m156)
struct ClipStrokeRectGM;

impl GM for ClipStrokeRectGM {
    fn name(&self) -> String {
        "clip_strokerect".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(200, 400)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut p = Paint::default();
        p.set_color(Color::RED);
        p.set_anti_alias(true);
        p.set_style(Style::Stroke);
        p.set_stroke_width(22.0);

        let r = Rect::from_xywh(20.0, 20.0, 100.0, 100.0);
        // setting the height of this to 19 causes failure
        let rect = Rect::from_xywh(20.0, 0.0, 100.0, 20.0);

        canvas.save();
        canvas.clip_rect(rect, None, true);
        canvas.draw_rect(r, &p);
        canvas.restore();

        p.set_color(Color::BLUE);
        p.set_stroke_width(2.0);
        canvas.draw_rect(rect, &p);

        p.set_color(Color::RED);
        p.set_anti_alias(true);
        p.set_style(Style::Stroke);
        p.set_stroke_width(22.0);

        let r2 = Rect::from_xywh(20.0, 140.0, 100.0, 100.0);
        // setting the height of this to 19 causes failure
        let rect2 = Rect::from_xywh(20.0, 120.0, 100.0, 19.0);

        canvas.save();
        canvas.clip_rect(rect2, None, true);
        canvas.draw_rect(r2, &p);
        canvas.restore();

        p.set_color(Color::BLUE);
        p.set_stroke_width(2.0);
        canvas.draw_rect(rect2, &p);
    }
}

// Port of: gm/clip_strokerect.cpp#L69 (chrome/m156)
crate::def_gm!(ClipStrokeRectGM_ = "ClipStrokeRectGM", ClipStrokeRectGM);
