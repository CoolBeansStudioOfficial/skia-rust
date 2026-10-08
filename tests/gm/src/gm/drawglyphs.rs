// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawglyphs.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::scalar::{SCALAR_PI, scalar};
use skia_rust_tools::font_tool_utils::create_portable_typeface;

// Port of: gm/drawglyphs.cpp#L19 (chrome/m156)
const TEXT: &str = "Call me Ishmael. Some years ago\u{2014}never mind how long precisely";

// Port of: gm/drawglyphs.cpp#L21-L83 (chrome/m156), DrawGlyphsGM
struct DrawGlyphsGm {
    font: Font,
    glyphs: Vec<GlyphId>,
    positions: Vec<Point>,
    xforms: Vec<RSXform>,
    glyph_count: usize,
    radius: scalar,
    length: scalar,
}

impl DrawGlyphsGm {
    // Port of: gm/drawglyphs.cpp#L21-L23 (chrome/m156), the member initialization
    fn new() -> Self {
        let typeface = create_portable_typeface(Some("serif"), FontStyle::default());
        let font = Font::from_typeface(Some(typeface));
        Self {
            font,
            glyphs: Vec::new(),
            positions: Vec::new(),
            xforms: Vec::new(),
            glyph_count: 0,
            radius: 0.0,
            length: 0.0,
        }
    }
}

impl GM for DrawGlyphsGm {
    fn name(&self) -> String {
        "drawglyphs".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/drawglyphs.cpp#L23-L54 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.font.set_subpixel(true);
        self.font.set_size(18.0);
        let text = TEXT.as_bytes();
        self.glyph_count = self.font.count_text(text, TextEncoding::UTF8);
        self.glyphs = vec![0; self.glyph_count];
        self.font
            .text_to_glyphs(text, TextEncoding::UTF8, &mut self.glyphs);
        self.positions = vec![Point::default(); self.glyph_count];
        self.font
            .get_pos(&self.glyphs, &mut self.positions, Point::default());

        let first = self.positions[0];
        let last = self.positions[self.glyph_count - 1];
        self.length = last.x - first.x;
        self.radius = self.length / SCALAR_PI;
        self.xforms = vec![RSXform::default(); self.glyph_count];
        for (xform, pos) in self.xforms.iter_mut().zip(&self.positions) {
            let length_to_glyph = pos.x - first.x;
            let angle = SCALAR_PI * (self.length - length_to_glyph) / self.length;
            let cos = angle.cos();
            let sin = angle.sin();
            *xform = RSXform::new(sin, cos, self.radius * cos, -self.radius * sin);
        }
    }

    // Port of: gm/drawglyphs.cpp#L55-L82 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let paint = Paint::default();
        canvas.draw_glyphs_at(
            &self.glyphs,
            &self.positions[..],
            (50.0, 100.0),
            &self.font,
            &paint,
        );
        canvas.draw_glyphs_at(
            &self.glyphs,
            &self.positions[..],
            (50.0, 120.0),
            &self.font,
            &paint,
        );
        // Check bounding box calculation.
        for p in &mut self.positions {
            p.y += -500.0;
        }
        canvas.draw_glyphs_at(
            &self.glyphs,
            &self.positions[..],
            (50.0, 640.0),
            &self.font,
            &paint,
        );
        canvas.draw_glyphs_at(
            &self.glyphs,
            &self.xforms[..],
            (50.0 + self.length / 2.0, 160.0 + self.radius),
            &self.font,
            &paint,
        );
        // TODO: add tests for cluster versions of drawGlyphs.
    }
}

// Port of: gm/drawglyphs.cpp#L85 (chrome/m156)
crate::def_gm!(DrawGlyphsGm_ = "DrawGlyphsGM{}", DrawGlyphsGm::new());
