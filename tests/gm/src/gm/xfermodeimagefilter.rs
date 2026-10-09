// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/xfermodeimagefilter.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: int/float conversions, local constants and long
// bodies are kept as they are there.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::too_many_lines
)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_effects::image_filters::{arithmetic, blend, image_sampled, offset};
use skia_rust_tools::font_tool_utils::create_string_bitmap;

const WIDTH: i32 = 600;
const HEIGHT: i32 = 700;
const MARGIN: i32 = 12;

/// `XfermodeImageFilterGM`.
// Port of: gm/xfermodeimagefilter.cpp#L17-L115 (chrome/m156)
struct XfermodeImageFilterGm {
    bitmap: Option<Bitmap>,
    checkerboard: Option<Image>,
}

/// `XfermodeImageFilterGM::DrawClippedBitmap`.
// Port of: gm/xfermodeimagefilter.cpp#L117-L124 (chrome/m156)
fn draw_clipped_bitmap(canvas: &Canvas, bitmap: &Bitmap, paint: &Paint, x: i32, y: i32) {
    canvas.save();
    canvas.translate((x as f32, y as f32));
    canvas.clip_irect(bitmap.bounds(), None);
    if let Some(image) = bitmap.as_image() {
        canvas.draw_image_with_sampling_options(
            &image,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(paint),
        );
    }
    canvas.restore();
}

/// `XfermodeImageFilterGM::DrawClippedPaint`.
// Port of: gm/xfermodeimagefilter.cpp#L126-L133 (chrome/m156)
fn draw_clipped_paint(canvas: &Canvas, rect: Rect, paint: &Paint, x: i32, y: i32) {
    canvas.save();
    canvas.translate((x as f32, y as f32));
    canvas.clip_rect(rect, None, None);
    canvas.draw_paint(paint);
    canvas.restore();
}

/// Moves the layout cursor to the next cell, wrapping to a new row when the next cell would not
/// fit (the `x += ...; if (x + width > WIDTH) { ... }` of the C++ loop).
fn advance(x: &mut i32, y: &mut i32, cell_width: i32, cell_height: i32) {
    *x += cell_width + MARGIN;
    if *x + cell_width > WIDTH {
        *x = 0;
        *y += cell_height + MARGIN;
    }
}

impl GM for XfermodeImageFilterGm {
    fn name(&self) -> String {
        "xfermodeimagefilter".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFF00_0000)
    }

    // Port of: gm/xfermodeimagefilter.cpp#L35-L38 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.bitmap = Some(create_string_bitmap(
            80,
            80,
            Color::new(0xD000_D000),
            15,
            65,
            96,
            "e",
        ));
        self.checkerboard = Some(crate::tool_utils::create_checkerboard_image(
            80,
            80,
            Color::new(0xFFA0_A0A0),
            Color::new(0xFF40_4040),
            8,
        ));
    }

    // Port of: gm/xfermodeimagefilter.cpp#L40-L115 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let (Some(bitmap), Some(checkerboard)) = (self.bitmap.as_ref(), self.checkerboard.as_ref())
        else {
            return;
        };
        let (bw, bh) = (bitmap.width(), bitmap.height());
        canvas.clear(Color::BLACK);
        let mut paint = Paint::default();
        let g_modes = [
            BlendMode::Clear,
            BlendMode::Src,
            BlendMode::Dst,
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcATop,
            BlendMode::DstATop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Modulate,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Multiply,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ];
        let mut x = 0;
        let mut y = 0;
        let nearest = SamplingOptions::new(FilterMode::Nearest, MipmapMode::None);
        let background: Option<ImageFilter> = image_sampled(Some(checkerboard.clone()), nearest);
        for mode in g_modes {
            paint.set_image_filter(blend(mode, background.clone(), None, None));
            draw_clipped_bitmap(canvas, bitmap, &paint, x, y);
            advance(&mut x, &mut y, bw, bh);
        }

        // Test arithmetic mode as image filter
        paint.set_image_filter(arithmetic(
            0.0,
            1.0,
            1.0,
            0.0,
            true,
            background.clone(),
            None,
            None,
        ));
        draw_clipped_bitmap(canvas, bitmap, &paint, x, y);
        advance(&mut x, &mut y, bw, bh);

        // Test nullptr mode
        paint.set_image_filter(blend(BlendMode::SrcOver, background.clone(), None, None));
        draw_clipped_bitmap(canvas, bitmap, &paint, x, y);
        advance(&mut x, &mut y, bw, bh);

        let clip_rect = Rect::from_wh((bw + 4) as f32, (bh + 4) as f32);

        // Test offsets on SrcMode (uses fixed-function blend)
        let bitmap_image = bitmap.as_image();
        let foreground: Option<ImageFilter> = image_sampled(bitmap_image, nearest);
        let offset_foreground = offset((4.0, -4.0), foreground.clone(), None);
        let offset_background = offset((4.0, 4.0), background.clone(), None);
        paint.set_image_filter(blend(
            BlendMode::SrcOver,
            offset_background.clone(),
            offset_foreground.clone(),
            None,
        ));
        draw_clipped_paint(canvas, clip_rect, &paint, x, y);
        advance(&mut x, &mut y, bw, bh);

        // Test offsets on Darken (uses shader blend)
        paint.set_image_filter(blend(
            BlendMode::Darken,
            offset_background.clone(),
            offset_foreground.clone(),
            None,
        ));
        draw_clipped_paint(canvas, clip_rect, &paint, x, y);
        advance(&mut x, &mut y, bw, bh);

        // Test cropping
        let sampled_modes = [BlendMode::Overlay, BlendMode::SrcOver, BlendMode::Plus];
        let offsets: [[i32; 4]; 3] = [[10, 10, -16, -16], [10, 10, 10, 10], [-10, -10, -6, -6]];
        for (mode, o) in sampled_modes.into_iter().zip(offsets) {
            let crop_rect = IRect::from_xywh(o[0], o[1], bw + o[2], bh + o[3]);
            paint.set_image_filter(blend(
                mode,
                offset_background.clone(),
                offset_foreground.clone(),
                Some(Rect::from_irect(crop_rect)),
            ));
            draw_clipped_paint(canvas, clip_rect, &paint, x, y);
            advance(&mut x, &mut y, bw, bh);
        }

        // Test small bg, large fg with Screen (uses shader blend)
        let crop_rect = IRect::from_xywh(10, 10, 60, 60);
        let cropped: Option<ImageFilter> = offset(
            (0.0, 0.0),
            foreground.clone(),
            Some(Rect::from_irect(crop_rect)),
        );
        paint.set_image_filter(blend(
            BlendMode::Screen,
            cropped.clone(),
            background.clone(),
            None,
        ));
        draw_clipped_paint(canvas, clip_rect, &paint, x, y);
        advance(&mut x, &mut y, bw, bh);

        // Test small fg, large bg with Screen (uses shader blend)
        paint.set_image_filter(blend(
            BlendMode::Screen,
            background.clone(),
            cropped.clone(),
            None,
        ));
        draw_clipped_paint(canvas, clip_rect, &paint, x, y);
        advance(&mut x, &mut y, bw, bh);

        // Test small fg, large bg with SrcIn with a crop that forces it to full size.
        // This tests that SkXfermodeImageFilter correctly applies the compositing mode to
        // the region outside the foreground.
        let crop_rect_full = IRect::from_xywh(0, 0, 80, 80);
        paint.set_image_filter(blend(
            BlendMode::SrcIn,
            background.clone(),
            cropped.clone(),
            Some(Rect::from_irect(crop_rect_full)),
        ));
        draw_clipped_paint(canvas, clip_rect, &paint, x, y);
        advance(&mut x, &mut y, bw, bh);
    }
}

// Port of: gm/xfermodeimagefilter.cpp#L206 (chrome/m156)
crate::def_gm!(
    XfermodeImageFilterGM,
    XfermodeImageFilterGm {
        bitmap: None,
        checkerboard: None,
    }
);
