// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_8955.cpp (chrome/m156)

use skia_rust_core::paint::Paint;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/skbug_8955.cpp#L10-L28 (chrome/m156), DEF_SIMPLE_GM(skbug_8955, canvas, 100, 100)
//
// This bug only appeared when drawing the same text blob. We would generate no glyphs on the
// first draw, and fail to mark the blob as having any bitmap runs. That would prevent us from
// re-generating the blob on the second draw, even though the matrix had been restored.
crate::def_simple_gm!(skbug_8955, canvas, 100, 100, {
    let p = Paint::default();
    let mut font = default_portable_font();
    font.set_size(50.0);
    let blob = TextBlob::from_str("+", &font);

    canvas.save();
    canvas.scale((0.0, 0.0));
    if let Some(blob) = &blob {
        canvas.draw_text_blob(blob, (30.0, 60.0), &p);
    }
    canvas.restore();
    if let Some(blob) = &blob {
        canvas.draw_text_blob(blob, (30.0, 60.0), &p);
    }
});
