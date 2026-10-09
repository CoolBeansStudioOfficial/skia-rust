// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobuseaftergpufree.cpp (chrome/m156)
//
// The GPU-only `freeGpuResources` step is compiled out (`SK_GANESH`), so the CPU build draws the
// blob twice without it.

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const WIDTH: i32 = 200;
const HEIGHT: i32 = 200;

// This tests that we correctly regenerate textblobs after freeing all gpu resources crbug/491350
// Port of: gm/textblobuseaftergpufree.cpp#L16-L43 (chrome/m156), TextBlobUseAfterGpuFree
struct TextBlobUseAfterGpuFreeGm;

impl GM for TextBlobUseAfterGpuFreeGm {
    // Port of: gm/textblobuseaftergpufree.cpp#L19 (chrome/m156), getName
    fn name(&self) -> String {
        "textblobuseaftergpufree".to_owned()
    }

    // Port of: gm/textblobuseaftergpufree.cpp#L20 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/textblobuseaftergpufree.cpp#L21-L40 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let text = "Hamburgefons";
        let font = Font::from_size(default_portable_typeface(), 20.0);
        let Some(blob) = TextBlob::from_text(text.as_bytes(), TextEncoding::UTF8, &font) else {
            return;
        };
        // draw textblob
        let rect = Rect::from_ltrb(0.0, 0.0, 200.0, 200.0 / 2.0);
        let mut rect_paint = Paint::default();
        rect_paint.set_color(Color::WHITE);
        canvas.draw_rect(rect, &rect_paint);
        canvas.draw_text_blob(&blob, (20.0, 60.0), &Paint::default());
        // The GPU-only freeGpuResources step (SK_GANESH) is not built here.
        canvas.draw_text_blob(&blob, (20.0, 160.0), &Paint::default());
    }
}

// Port of: gm/textblobuseaftergpufree.cpp#L43 (chrome/m156), DEF_GM
crate::def_gm!(TextBlobUseAfterGpuFree, TextBlobUseAfterGpuFreeGm);
