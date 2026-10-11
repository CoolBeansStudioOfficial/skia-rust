// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/overdrawcanvas.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_effects::overdraw_color_filter;
use skia_rust_raster::overdraw_canvas::OverdrawCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_font;

const WIDTH: i32 = 500;
const HEIGHT: i32 = 500;

// Port of: gm/overdrawcanvas.cpp#L28-L30 (chrome/m156), kOverdrawColors
const OVERDRAW_COLORS: [Color; overdraw_color_filter::NUM_COLORS] = [
    Color::new(0x0000_0000),
    Color::new(0x5fff_0000),
    Color::new(0x2f00_00ff),
    Color::new(0x2f00_ff00),
    Color::new(0x3fff_0000),
    Color::new(0x7fff_0000),
];

// Port of: gm/overdrawcanvas.cpp#L33-L59 (chrome/m156), DEF_SIMPLE_GM_BG(overdraw_canvas, ...)
crate::def_simple_gm_bg!(overdraw_canvas, canvas, WIDTH, HEIGHT, Color::WHITE, {
    // Set up the overdraw canvas.
    let offscreen_info = ImageInfo::new_a8((WIDTH, HEIGHT));
    let Some(mut offscreen) = surfaces::raster(&offscreen_info, None, None) else {
        return;
    };
    let font = default_portable_font();
    let c = offscreen.shared_canvas();

    let overdraw_canvas = OverdrawCanvas::new(c);

    overdraw_canvas.draw_rect(&Rect::new(10.0, 10.0, 200.0, 200.0), &Paint::default());
    overdraw_canvas.draw_rect(&Rect::new(20.0, 20.0, 190.0, 190.0), &Paint::default());
    overdraw_canvas.draw_rect(&Rect::new(30.0, 30.0, 180.0, 180.0), &Paint::default());
    overdraw_canvas.draw_rect(&Rect::new(40.0, 40.0, 170.0, 170.0), &Paint::default());
    overdraw_canvas.draw_rect(&Rect::new(50.0, 50.0, 160.0, 160.0), &Paint::default());
    overdraw_canvas.draw_rect(&Rect::new(60.0, 60.0, 150.0, 150.0), &Paint::default());

    let text = b"Ae_p";
    overdraw_canvas.draw_simple_text(
        text,
        TextEncoding::UTF8,
        (300.0, 300.0),
        &font,
        &Paint::default(),
    );

    let counts = offscreen.image_snapshot();

    // Draw overdraw colors to the canvas.  The color filter will convert counts to colors.
    let mut paint = Paint::default();
    paint.set_color_filter(overdraw_color_filter::make_with_sk_colors(&OVERDRAW_COLORS));

    if let Some(counts) = counts {
        canvas.draw_image_with_sampling_options(
            &counts,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&paint),
        );
    }
    canvas.draw_str(
        "This is some text:",
        (180.0, 300.0),
        &font,
        &Paint::default(),
    );
});

// Port of: gm/overdrawcanvas.cpp#L61-L87 (chrome/m156), overdraw_text_grid
fn overdraw_text_grid(use_ctm: bool) -> Option<Image> {
    let mut surface = surfaces::raster(&ImageInfo::new_a8((256, 512)), None, None)?;
    let canvas = OverdrawCanvas::new(surface.shared_canvas());

    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);

    let font = default_portable_font();

    for n in 1..=20_i32 {
        let x = 10.0_f32;
        let y = int_to_scalar(n) * 20.0;

        for _ in 0..n * 10 {
            let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
            if use_ctm {
                canvas.save();
                canvas.translate((x, y));
                canvas.draw_str(text, (0.0, 0.0), &font, &paint);
                canvas.restore();
            } else {
                canvas.draw_str(text, (x, y), &font, &paint);
            }
        }
    }
    surface.image_snapshot()
}

// This GM tests the underlying problem from skbug.com/40044818. Text drawn through an overdraw
// canvas would have the CTM applied twice. If everything is working, both images generated should
// look identical. When the bug was present, the second image would have the lines "double spaced",
// because the translations were applied twice.
// Port of: gm/overdrawcanvas.cpp#L93-L98 (chrome/m156), DEF_SIMPLE_GM_BG(overdraw_text_xform, ...)
crate::def_simple_gm_bg!(overdraw_text_xform, canvas, 512, 512, Color::BLACK, {
    let mut img_paint = Paint::default();
    img_paint.set_color(Color::WHITE);
    if let Some(img) = overdraw_text_grid(false) {
        canvas.draw_image_with_sampling_options(
            &img,
            (0.0, 0.0),
            FilterMode::Nearest,
            Some(&img_paint),
        );
    }
    if let Some(img) = overdraw_text_grid(true) {
        canvas.draw_image_with_sampling_options(
            &img,
            (256.0, 0.0),
            FilterMode::Nearest,
            Some(&img_paint),
        );
    }
});
