// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pdf_never_embed.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_tools::font_tool_utils::{
    create_typeface_from_resource, default_portable_typeface, test_font_mgr,
};
use skia_rust_tools::resources::get_resource_as_stream;

// Port of: gm/pdf_never_embed.cpp#L22-L33 (chrome/m156)
fn excercise_draw_pos_text(
    canvas: &Canvas,
    text: &str,
    x: scalar,
    y: scalar,
    font: &Font,
    paint: &Paint,
) {
    let count = font.count_text(text.as_bytes(), TextEncoding::UTF8);
    let mut builder = TextBlobBuilder::new();
    let (glyphs, points) = builder.alloc_run_pos(font, count, None);
    font.text_to_glyphs(text.as_bytes(), TextEncoding::UTF8, glyphs);
    font.get_pos(glyphs, points, Point::new(0.0, 0.0));
    let blob = builder.make().expect("a text blob");
    canvas.draw_text_blob(&blob, (x, y), paint);
}

// Port of: gm/pdf_never_embed.cpp#L39-L71 (chrome/m156)
crate::def_simple_gm_can_fail!(pdf_never_embed, canvas, error_msg, 512, 512, {
    let mut p = Paint::default();

    let tf = create_typeface_from_resource(
        get_resource_as_stream("fonts/Roboto2-Regular_NoEmbed.ttf"),
        0,
    )
    .unwrap_or_else(default_portable_typeface);
    let font = Font::from_size(tf, 60.0);

    let text = "HELLO, WORLD!";

    canvas.draw_color(Color::WHITE, None);
    excercise_draw_pos_text(canvas, text, 30.0, 90.0, &font, &p);

    canvas.save();
    canvas.rotate(45.0, None);
    p.set_color(Color::from(0xF080_0000));
    excercise_draw_pos_text(canvas, text, 30.0, 45.0, &font, &p);
    canvas.restore();

    canvas.save();
    canvas.scale((1.0, 4.0));
    p.set_color(Color::from(0xF000_8000));
    excercise_draw_pos_text(canvas, text, 15.0, 70.0, &font, &p);
    canvas.restore();

    canvas.scale((1.0, 0.5));
    p.set_color(Color::from(0xF000_0080));
    canvas.draw_simple_text(text, TextEncoding::UTF8, (30.0, 700.0), &font, &p);
    DrawResult::Ok
});

// should draw completely white.
// Port of: gm/pdf_never_embed.cpp#L75-L82 (chrome/m156)
crate::def_simple_gm!(pdf_crbug_772685, canvas, 612, 792, {
    canvas.clip_rect(Rect::new(-1.0, -1.0, 613.0, 793.0), None, false);
    canvas.translate((-571.0, 0.0));
    canvas.scale((0.75, 0.75));
    canvas.clip_rect(Rect::new(-1.0, -1.0, 613.0, 793.0), None, false);
    canvas.translate((0.0, -816.0));
    canvas.draw_rect(Rect::new(0.0, 0.0, 1224.0, 1500.0), &Paint::default());
});

// See https://issues.skia.org/issues/40045290
// This has two uses. The first is to simply show the behavior of the various font managers.
// The second is to test the behavior of the PDF subsetter for OpenType fonts which are table based.
// Port of: gm/pdf_never_embed.cpp#L87-L128 (chrome/m156)
crate::def_simple_gm_can_fail!(pdf_table_based_subset, canvas, error_msg, 512, 128, {
    let p = Paint::default();

    let fm = test_font_mgr();

    // `GetResourceAsStream(resource, useStream)`: with `useStream` true the C++ gives a file
    // stream instead of a memory stream; both read the same bytes.
    let make_typeface = |resource: &str| {
        fm.make_from_stream(get_resource_as_stream(resource), 0)
            .unwrap_or_else(default_portable_typeface)
    };
    let typefaces = [
        make_typeface("fonts/SpiderSymbol.ttf"),
        make_typeface("fonts/SpiderSymbol.ttf"),
        make_typeface("fonts/SpiderSymbol.woff"),
        make_typeface("fonts/SpiderSymbol.woff"),
        make_typeface("fonts/SpiderSymbol.woff2"),
        make_typeface("fonts/SpiderSymbol.woff2"),
    ];

    let mut o = Point::new(10.0, 10.0);
    let text = "\u{1F577}"; // {0xF0, 0x9F, 0x95, 0xB7}
    let cluster: u32 = 0;
    let spider_code_point = 0xf021;
    for tf in &typefaces {
        let font = Font::from_size(tf.clone(), 60.0);
        let mut g = font.unichar_to_glyph(spider_code_point);
        if g == 0 {
            // Avoid adding glyph 0 since it can be difficult to tell apart from the default.
            g = font.unichar_to_glyph('S' as i32);
        }
        let mut bounds = [Rect::default()];
        font.get_widths_bounds(&[g], &mut [], &mut bounds, Some(&p));
        let bounds = bounds[0];

        let pt = Point::new(-bounds.left, -bounds.top);
        canvas.draw_glyphs_utf8(&[g], &[pt], &[cluster], text, o, &font, &p);
        o.offset((bounds.width() + 10.0, 0.0));
    }

    DrawResult::Ok
});
