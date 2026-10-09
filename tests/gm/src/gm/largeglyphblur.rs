// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/largeglyphblur.cpp (chrome/m156)

use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/largeglyphblur.cpp#L14-L36 (chrome/m156), largeglyphblur
crate::def_simple_gm!(largeglyphblur, canvas, 1920, 600, {
    let text = "Hamburgefons";

    let font = Font::from_size(default_portable_typeface(), 256.0);
    let blob = TextBlob::from_text(text.as_bytes(), TextEncoding::UTF8, &font)
        .expect("the text has glyphs");

    // setup up maskfilter
    let sigma = BlurMask::convert_radius_to_sigma(40.0);

    let mut blur_paint = Paint::default();
    blur_paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma, None));

    canvas.draw_text_blob(&blob, (10.0, 200.0), &blur_paint);
    canvas.draw_text_blob(&blob, (10.0, 200.0), &Paint::default());

    canvas.draw_simple_text(
        text.as_bytes(),
        TextEncoding::UTF8,
        (10.0, 500.0),
        &font,
        &blur_paint,
    );
    canvas.draw_simple_text(
        text.as_bytes(),
        TextEncoding::UTF8,
        (10.0, 500.0),
        &font,
        &Paint::default(),
    );
});
