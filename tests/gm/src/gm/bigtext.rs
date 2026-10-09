// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bigtext.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_tools::font_tool_utils::{create_typeface_from_resource, default_portable_typeface};

// Port of: gm/bigtext.cpp#L29-L59 (chrome/m156), BigTextGM
struct BigTextGm;

impl GM for BigTextGm {
    fn name(&self) -> String {
        "bigtext".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // onDraw of BigTextGM
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let font = Font::from_size(default_portable_typeface(), 1500.0);
        let (_, r) = font.measure_text(b"/", TextEncoding::UTF8, None);
        // `this->width()/2` and `height()/2` are integer divisions of 640 and 480.
        let pos = Point::new(320.0 - r.center_x(), 240.0 - r.center_y());
        paint.set_color(Color::RED);
        canvas.draw_simple_text(b"/", TextEncoding::UTF8, pos, &font, &paint);
        paint.set_color(Color::BLUE);
        canvas.draw_simple_text(b"\\", TextEncoding::UTF8, pos, &font, &paint);
    }
}

// Port of: gm/bigtext.cpp#L61 (chrome/m156)
crate::def_gm!(BigTextGM, BigTextGm);

// Port of: gm/bigtext.cpp#L66-L80 (chrome/m156), bigtext_crbug_1370488
crate::def_simple_gm!(bigtext_crbug_1370488, canvas, 512, 512, {
    // The Spider symbol font is a resource, so this always takes the portable fallback.
    let (typeface, text): (_, &[u8]) = match create_typeface_from_resource(None, 0) {
        Some(typeface) => (typeface, b"\xEF\x80\xA1"),
        None => (default_portable_typeface(), b"H"),
    };
    let font = Font::from_size(typeface, 12.0);
    canvas.translate((-1800.0, 1800.0));
    canvas.scale((437.5, 437.5));
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_simple_text(text, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
});
