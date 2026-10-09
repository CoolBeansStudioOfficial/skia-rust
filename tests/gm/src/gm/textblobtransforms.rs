// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobtransforms.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar_ceil_to_scalar;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_typeface};

const WIDTH: i32 = 1000;
const HEIGHT: i32 = 1200;

// Port of: gm/textblobtransforms.cpp#L16-L63 (chrome/m156), TextBlobTransforms
struct TextBlobTransformsGm {
    blob: Option<TextBlob>,
}

impl GM for TextBlobTransformsGm {
    // Port of: gm/textblobtransforms.cpp#L23-L47 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut builder = TextBlobBuilder::new();
        // make textblob.  To stress distance fields, we choose sizes appropriately
        let mut font = Font::from_size(default_portable_typeface(), 162.0);
        font.set_edging(Edging::Alias);
        let mut text = "A";
        let (_, mut bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        add_to_text_blob(&mut builder, text, &font, 0.0, 0.0);
        // Medium
        let x_offset = bounds.width() + 5.0;
        font.set_size(72.0);
        text = "B";
        add_to_text_blob(&mut builder, text, &font, x_offset, 0.0);
        (_, bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        let y_offset = bounds.height();
        // Small
        font.set_size(32.0);
        text = "C";
        add_to_text_blob(&mut builder, text, &font, x_offset, -y_offset - 10.0);
        // build
        self.blob = builder.make();
    }

    // Port of: gm/textblobtransforms.cpp#L49 (chrome/m156), getName
    fn name(&self) -> String {
        "textblobtransforms".to_owned()
    }

    // Port of: gm/textblobtransforms.cpp#L50 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/textblobtransforms.cpp#L51-L121 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(blob) = self.blob.clone() else {
            return;
        };
        canvas.draw_color(Color::GRAY, None);
        let paint = Paint::default();
        let bounds = *blob.bounds();
        canvas.translate((20.0, 20.0));
        // The GM's offsets are the blob's ceiled size.
        let x_offset = scalar_ceil_to_scalar(bounds.width());
        let y_offset = scalar_ceil_to_scalar(bounds.height());
        // first translate
        canvas.translate((x_offset, 2.0 * y_offset));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.translate((-x_offset, 0.0));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.translate((2.0 * x_offset, 0.0));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.translate((-x_offset, -y_offset));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.translate((0.0, 2.0 * y_offset));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        // now rotate
        canvas.translate((4.0 * x_offset, -y_offset));
        canvas.rotate(180.0, None);
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(-180.0, None);
        canvas.translate((0.0, -y_offset));
        canvas.rotate(-180.0, None);
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(270.0, None);
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(-90.0, None);
        canvas.translate((-x_offset, y_offset));
        canvas.rotate(-90.0, None);
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(90.0, None);
        // and scales
        canvas.translate((-3.0 * x_offset, 3.0 * y_offset));
        canvas.scale((1.5, 1.5));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.translate((x_offset, 0.0));
        canvas.scale((0.25, 0.25));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.translate((x_offset, 0.0));
        canvas.scale((3.0, 2.0));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        // finally rotates, scales, and translates together
        canvas.translate((x_offset, 0.0));
        canvas.rotate(23.0, None);
        canvas.scale((0.33, 0.5));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(-46.0, None);
        canvas.translate((x_offset, 0.0));
        canvas.scale((1.2, 1.1));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(46.0, None);
        canvas.translate((x_offset, 0.0));
        canvas.scale((1.1, 1.2));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(46.0, None);
        canvas.translate((x_offset, 0.0));
        canvas.scale((0.95, 1.1));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(46.0, None);
        canvas.translate((x_offset, 0.0));
        canvas.scale((1.3, 0.7));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(46.0, None);
        canvas.translate((x_offset, 0.0));
        canvas.scale((0.8, 1.1));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(10.0, None);
        canvas.translate((x_offset, 0.0));
        canvas.scale((1.0, 5.0));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.rotate(5.0, None);
        canvas.translate((x_offset, 0.0));
        canvas.scale((5.0, 1.0));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
    }
}

// Port of: gm/textblobtransforms.cpp#L175 (chrome/m156), DEF_GM
crate::def_gm!(TextBlobTransforms, TextBlobTransformsGm { blob: None });
