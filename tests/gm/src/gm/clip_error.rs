// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/clip_error.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/clip_error.cpp#L1-L2 (chrome/m156)
const WIDTH: i32 = 800;
// Port of: gm/clip_error.cpp#L1-L2 (chrome/m156)
const HEIGHT: i32 = 800;

// Port of: gm/clip_error.cpp#L4-L13 (chrome/m156)
fn draw_text(
    canvas: &Canvas,
    blob: &TextBlob,
    paint: &Paint,
    blur_paint: &Paint,
    clear_paint: &Paint,
) {
    canvas.save();
    canvas.clip_rect(Rect::from_ltrb(0.0, 0.0, 1081.0, 665.0), None, None);
    canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 1081.0, 665.0), clear_paint);
    canvas.draw_text_blob(blob, Point::new(0.0, 256.0), blur_paint);
    canvas.draw_text_blob(blob, Point::new(0.0, 477.0), paint);
    canvas.restore();
}

// Port of: gm/clip_error.cpp#L15-L49 (chrome/m156)
struct ClipErrorGm;

impl GM for ClipErrorGm {
    fn name(&self) -> String {
        "cliperror".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/clip_error.cpp#L23-L44 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let font = Font::from_size(default_portable_typeface(), 256.0);
        let k_sigma = BlurMask::convert_radius_to_sigma(50.0);
        let mut blur_paint = paint.clone();
        blur_paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, k_sigma, None));
        let text = b"hambur";
        let blob = TextBlob::from_text(text, TextEncoding::UTF8, &font).expect("a text blob");
        let mut clear_paint = paint.clone();
        clear_paint.set_color(Color::WHITE);

        canvas.save();
        canvas.translate((0.0, 0.0));
        canvas.clip_rect(Rect::from_ltrb(0.0, 0.0, WIDTH as f32, 256.0), None, None);
        draw_text(canvas, &blob, &paint, &blur_paint, &clear_paint);
        canvas.restore();
        canvas.save();
        canvas.translate((0.0, 256.0));
        canvas.clip_rect(Rect::from_ltrb(0.0, 256.0, WIDTH as f32, 510.0), None, None);
        draw_text(canvas, &blob, &paint, &blur_paint, &clear_paint);
        canvas.restore();
    }
}

crate::def_gm!(ClipErrorGM, ClipErrorGm);
