// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/highcontrastfilter.cpp (chrome/m156)
//
// The `fFilter`, `fGr1` and `fGr2` members that `onOnceBeforeDraw` builds are never read by
// `onDraw`, so they are not ported: they cannot change the drawn pixels.

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::font::Edging;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::high_contrast_filter::{
    HighContrastConfig, HighContrastFilter, InvertStyle,
};
use skia_rust_tools::font_tool_utils::default_portable_font;

const K_SIZE: f32 = 200.0;

// Port of: gm/highcontrastfilter.cpp#L14-L28 (chrome/m156), draw_label
fn draw_label(canvas: &Canvas, config: &HighContrastConfig) {
    let invert_str = if config.invert_style == InvertStyle::INVERT_BRIGHTNESS {
        "InvBright"
    } else if config.invert_style == InvertStyle::INVERT_LIGHTNESS {
        "InvLight"
    } else {
        "NoInvert"
    };
    let label = format!(
        "{}{} contrast={:.1}",
        if config.grayscale { "Gray " } else { "" },
        invert_str,
        config.contrast
    );
    let mut font = default_portable_font();
    font.set_size(0.075);
    font.set_edging(Edging::AntiAlias);
    let (width, _) = font.measure_text(label.as_bytes(), TextEncoding::UTF8, None);
    canvas.draw_simple_text(
        label.as_bytes(),
        TextEncoding::UTF8,
        (0.5 - width / 2.0, 0.16),
        &font,
        &Paint::default(),
    );
}

// Port of: gm/highcontrastfilter.cpp#L30-L69 (chrome/m156), draw_scene
fn draw_scene(canvas: &Canvas, config: &HighContrastConfig) {
    let mut bounds = Rect::new(0.0, 0.0, 1.0, 1.0);
    let mut xfer_paint = Paint::default();
    xfer_paint.set_color_filter(HighContrastFilter::make(config));
    canvas.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&xfer_paint));

    let mut paint = Paint::default();
    bounds = Rect::new(0.1, 0.2, 0.9, 0.4);
    paint.set_argb(0xff, 0x66, 0x11, 0x11);
    canvas.draw_rect(bounds, &paint);

    let mut font = default_portable_font();
    font.set_size(0.15);
    font.set_edging(Edging::Alias);
    paint.set_argb(0xff, 0xbb, 0x77, 0x77);
    canvas.draw_str("A", (0.15, 0.35), &font, &paint);

    bounds = Rect::new(0.1, 0.8, 0.9, 1.0);
    paint.set_argb(0xff, 0xcc, 0xcc, 0xff);
    canvas.draw_rect(bounds, &paint);
    paint.set_argb(0xff, 0x88, 0x88, 0xbb);
    canvas.draw_str("Z", (0.75, 0.95), &font, &paint);

    bounds = Rect::new(0.1, 0.4, 0.9, 0.6);
    let pts = [Point::new(0.0, 0.0), Point::new(1.0, 0.0)];
    let colors = [Color4f::from(Color::WHITE), Color4f::from(Color::BLACK)];
    let pos = [0.2_f32, 0.8];
    paint.set_shader(gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_rect(bounds, &paint);

    bounds = Rect::new(0.1, 0.6, 0.9, 0.8);
    let colors2 = [Color4f::from(Color::GREEN), Color4f::from(Color::WHITE)];
    paint.set_shader(gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors2, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_rect(bounds, &paint);

    canvas.restore();
}

// Port of: gm/highcontrastfilter.cpp#L71-L100 (chrome/m156), HighContrastFilterGM
struct HighContrastFilterGm;

impl GM for HighContrastFilterGm {
    fn name(&self) -> String {
        "highcontrastfilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 420)
    }

    // Port of: gm/highcontrastfilter.cpp#L85-L99 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let configs = [
            HighContrastConfig {
                grayscale: false,
                invert_style: InvertStyle::NO_INVERT,
                contrast: 0.0,
            },
            HighContrastConfig {
                grayscale: false,
                invert_style: InvertStyle::INVERT_BRIGHTNESS,
                contrast: 0.0,
            },
            HighContrastConfig {
                grayscale: false,
                invert_style: InvertStyle::INVERT_LIGHTNESS,
                contrast: 0.0,
            },
            HighContrastConfig {
                grayscale: false,
                invert_style: InvertStyle::INVERT_LIGHTNESS,
                contrast: 0.2,
            },
            HighContrastConfig {
                grayscale: true,
                invert_style: InvertStyle::NO_INVERT,
                contrast: 0.0,
            },
            HighContrastConfig {
                grayscale: true,
                invert_style: InvertStyle::INVERT_BRIGHTNESS,
                contrast: 0.0,
            },
            HighContrastConfig {
                grayscale: true,
                invert_style: InvertStyle::INVERT_LIGHTNESS,
                contrast: 0.0,
            },
            HighContrastConfig {
                grayscale: true,
                invert_style: InvertStyle::INVERT_LIGHTNESS,
                contrast: 0.2,
            },
        ];
        for (i, config) in (0_i32..).zip(configs.iter()) {
            let x = K_SIZE * int_to_scalar(i % 4);
            let y = K_SIZE * int_to_scalar(i / 4);
            canvas.save();
            canvas.translate((x, y));
            canvas.scale((K_SIZE, K_SIZE));
            draw_scene(canvas, config);
            draw_label(canvas, config);
            canvas.restore();
        }
    }
}

// Port of: gm/highcontrastfilter.cpp#L102 (chrome/m156)
crate::def_gm!(HighContrastFilterGM, HighContrastFilterGm);
