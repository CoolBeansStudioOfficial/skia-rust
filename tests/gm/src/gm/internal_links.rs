// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/internal_links.cpp (chrome/m156)

// Draws two rectangles. In output formats that support internal links (PDF),
// clicking the one labeled "Link to A" should take you to the one labeled
// "Target A". The SkAnnotate* calls are not ported: a raster device's drawAnnotation is empty
// (src/core/SkDevice.h), so they change no pixels.

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/internal_links.cpp#L25-L73 (chrome/m156)
#[derive(Debug)]
pub struct InternalLinksGm;

impl InternalLinksGm {
    // Port of: gm/internal_links.cpp#L27-L60 (chrome/m156), drawLabeledRect
    fn draw_labeled_rect(canvas: &Canvas, text: &str, x: f32, y: f32) {
        let mut paint = Paint::default();
        paint.set_color(Color::BLUE);
        let rect = Rect::from_xywh(x, y, 50.0, 20.0);
        canvas.draw_rect(rect, &paint);

        let font = Font::from_size(default_portable_typeface(), 25.0);
        paint.set_color(Color::BLACK);
        canvas.draw_str(text, (x, y), &font, &paint);
    }
}

impl GM for InternalLinksGm {
    fn name(&self) -> String {
        "internal_links".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(700, 500)
    }

    fn bg_color(&self) -> Color {
        Color::from_argb(0xFF, 0xDD, 0xDD, 0xDD)
    }

    // Port of: gm/internal_links.cpp#L37-L53 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.save();
        canvas.translate((100.0, 100.0));
        Self::draw_labeled_rect(canvas, "Link to A", 0.0, 0.0);
        canvas.restore();

        canvas.save();
        canvas.translate((200.0, 200.0));
        Self::draw_labeled_rect(canvas, "Target A", 100.0, 50.0);
        canvas.restore();
    }
}

// Port of: gm/internal_links.cpp#L71-L71 (chrome/m156)
crate::def_gm!(InternalLinksGM, InternalLinksGm);
