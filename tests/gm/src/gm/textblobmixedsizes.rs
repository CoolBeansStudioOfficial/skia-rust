// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobmixedsizes.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::m44::M44;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{int_to_scalar, scalar_floor_to_int};
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_typeface};

// Port of: gm/textblobmixedsizes.cpp#L17-L30 (chrome/m156), class TextBlobMixedSizes
struct TextBlobMixedSizesGm {
    use_dft: bool,
    blob: Option<TextBlob>,
}

const WIDTH: i32 = 2100;
const HEIGHT: i32 = 1900;

impl TextBlobMixedSizesGm {
    // Port of: gm/textblobmixedsizes.cpp#L20 (chrome/m156), TextBlobMixedSizes(bool useDFT)
    fn new(use_dft: bool) -> Self {
        Self {
            use_dft,
            blob: None,
        }
    }
}

impl GM for TextBlobMixedSizesGm {
    // Port of: gm/textblobmixedsizes.cpp#L69-L72 (chrome/m156), getName
    fn name(&self) -> String {
        if self.use_dft {
            "textblobmixedsizes_df".to_owned()
        } else {
            "textblobmixedsizes".to_owned()
        }
    }

    // Port of: gm/textblobmixedsizes.cpp#L73 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/textblobmixedsizes.cpp#L22-L67 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut builder = TextBlobBuilder::new();
        // The fonts/HangingS.ttf resource is not available here, so the portable typeface is
        // used, as C++ does when the resource is null.
        let typeface = default_portable_typeface();
        // make textblob.  To stress distance fields, we choose sizes appropriately
        let mut font = Font::from_size(typeface, 262.0);
        font.set_subpixel(true);
        font.set_edging(Edging::SubpixelAntiAlias);
        let text = "Skia";
        add_to_text_blob(&mut builder, text, &font, 0.0, 0.0);

        // Each size is measured at the size it was drawn at, and the baseline moves down by the
        // height of the glyph bounds.
        let height = |font: &Font| {
            font.measure_text(text.as_bytes(), TextEncoding::UTF8, None)
                .1
                .height()
        };
        let mut y_offset = height(&font);
        font.set_size(162.0); // large
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset);
        y_offset += height(&font);
        font.set_size(72.0); // Medium
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset);
        y_offset += height(&font);
        font.set_size(32.0); // Small
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset);
        // micro (will fall out of distance field text even if distance field text is enabled)
        y_offset += height(&font);
        font.set_size(14.0);
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset);
        // Zero size.
        y_offset += height(&font);
        font.set_size(0.0);
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset);

        // build
        self.blob = builder.make();
    }

    // Port of: gm/textblobmixedsizes.cpp#L74-L131 (chrome/m156), onDraw
    fn on_draw(&mut self, input_canvas: &Canvas) {
        // With DFT, C++ draws into an offscreen render target made from the recording context.
        // A CPU canvas has none, so the offscreen surface is null and the input canvas is used.
        let canvas = input_canvas;
        if self.use_dft {
            // init our new canvas with the old canvas's matrix
            canvas.set_matrix(&M44::from(canvas.total_matrix()));
        }
        canvas.draw_color(Color::WHITE, None);
        let Some(blob) = self.blob.as_ref() else {
            return;
        };
        let bounds = *blob.bounds();
        let pad_x = scalar_floor_to_int(bounds.width() / 3.0);
        let pad_y = scalar_floor_to_int(bounds.height() / 3.0);
        let mut row_count: i32 = 0;
        canvas.translate((int_to_scalar(pad_x), int_to_scalar(pad_y)));
        canvas.save();
        let mut random = Random::default();
        let mut paint = Paint::default();
        if !self.use_dft {
            paint.set_color(Color::WHITE);
        }
        paint.set_anti_alias(false);
        let sigma = BlurMask::convert_radius_to_sigma(int_to_scalar(8));
        // setup blur paint
        let mut blur_paint = paint.clone();
        blur_paint.set_color(Color::BLACK);
        blur_paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma, None));
        for i in 0..4 {
            canvas.save();
            match i % 2 {
                0 => {
                    canvas.rotate(random.next_f() * 45.0, None);
                }
                _ => {
                    canvas.rotate(-random.next_f() * 45.0, None);
                }
            }
            if !self.use_dft {
                canvas.draw_text_blob(blob, (0.0, 0.0), &blur_paint);
            }
            canvas.draw_text_blob(blob, (0.0, 0.0), &paint);
            canvas.restore();
            canvas.translate((bounds.width() + int_to_scalar(pad_x), 0.0));
            row_count += 1;
            if (bounds.width() + int_to_scalar(2 * pad_x)) * int_to_scalar(row_count)
                > int_to_scalar(WIDTH)
            {
                canvas.restore();
                canvas.translate((0.0, bounds.height() + int_to_scalar(pad_y)));
                canvas.save();
                row_count = 0;
            }
        }
        canvas.restore();
        // render offscreen buffer: there is none on the CPU (see above).
    }
}

// Port of: gm/textblobmixedsizes.cpp#L133-L146 (chrome/m156), DEF_GM(TextBlobMixedSizes(false))
crate::def_gm!(
    TextBlobMixedSizesGM = "textblobmixedsizes",
    TextBlobMixedSizesGm::new(false)
);
// Port of: gm/textblobmixedsizes.cpp#L133-L146 (chrome/m156), DEF_GM(TextBlobMixedSizes(true))
crate::def_gm!(
    TextBlobMixedSizesDftGM = "textblobmixedsizes_df",
    TextBlobMixedSizesGm::new(true)
);
