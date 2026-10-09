// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawatlascolor.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::needless_range_loop,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/drawatlascolor.cpp#L9-L30 (chrome/m156)
fn make_atlas(caller: &Canvas, atlas_size: i32) -> Option<Image> {
    let block_size = atlas_size / 2;
    let info = ImageInfo::new_n32_premul((atlas_size, atlas_size), None);
    let mut surface = caller
        .new_surface(&info, None)
        .or_else(|| surfaces::raster(&info, None, None))?;
    let bs = block_size as f32;
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    paint.set_color(Color::WHITE);
    let canvas = surface.canvas();
    let mut r = Rect::from_xywh(0.0, 0.0, bs, bs);
    canvas.draw_rect(r, &paint);
    paint.set_color(Color::RED);
    r = Rect::from_xywh(bs, 0.0, bs, bs);
    canvas.draw_rect(r, &paint);
    paint.set_color(Color::GREEN);
    r = Rect::from_xywh(0.0, bs, bs, bs);
    canvas.draw_rect(r, &paint);
    paint.set_color(Color::TRANSPARENT);
    r = Rect::from_xywh(bs, bs, bs, bs);
    canvas.draw_rect(r, &paint);
    surface.image_snapshot()
}

// Port of: gm/drawatlascolor.cpp#L31-L158 (chrome/m156)
const K_NUM_XFER_MODES: usize = 29;
const K_NUM_COLORS: usize = 4;
const K_ATLAS_SIZE: i32 = 30;
const K_PAD: i32 = 2;
const K_TEXT_PAD: i32 = 8;

struct DrawAtlasColorsGm;

impl GM for DrawAtlasColorsGm {
    fn name(&self) -> String {
        "draw-atlas-colors".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(
            K_NUM_XFER_MODES as i32 * (K_ATLAS_SIZE + K_PAD) + K_PAD,
            2 * K_NUM_COLORS as i32 * (K_ATLAS_SIZE + K_PAD) + K_TEXT_PAD + K_PAD,
        )
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFCCCCCC)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let target = Rect::from_wh(K_ATLAS_SIZE as f32, K_ATLAS_SIZE as f32);
        let atlas = make_atlas(canvas, K_ATLAS_SIZE).expect("an atlas");
        let modes: [BlendMode; K_NUM_XFER_MODES] = [
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
        let colors: [Color; K_NUM_COLORS] = [
            Color::WHITE,
            Color::RED,
            Color::new(0x88888888), // transparent grey
            Color::new(0x88000088), // transparent blue
        ];
        let mut xforms = [RSXform::default(); K_NUM_COLORS];
        let mut rects = [target; K_NUM_COLORS];
        let mut quad_colors = [Color::BLACK; K_NUM_COLORS];
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        for i in 0..K_NUM_COLORS {
            xforms[i].set(
                1.0,
                0.0,
                (K_PAD as f32, i as f32 * (target.width() + K_PAD as f32)),
            );
            rects[i] = target;
            quad_colors[i] = colors[i];
        }
        let font = Font::from_size(default_portable_typeface(), K_TEXT_PAD as f32);
        for (i, mode) in modes.iter().enumerate() {
            let label = mode.name();
            draw_string(
                canvas,
                label,
                i as f32 * (target.width() + K_PAD as f32) + K_PAD as f32,
                K_TEXT_PAD as f32,
                &font,
                &paint,
                Align::Left,
            );
        }
        for (i, mode) in modes.iter().enumerate() {
            canvas.save();
            canvas.translate((
                i as f32 * (target.height() + K_PAD as f32),
                (K_TEXT_PAD + K_PAD) as f32,
            ));
            canvas.draw_atlas(
                &atlas,
                &xforms,
                &rects,
                Some(&quad_colors[..]),
                *mode,
                SamplingOptions::default(),
                None,
                None,
            );
            canvas.translate((0.0, K_NUM_COLORS as f32 * (target.height() + K_PAD as f32)));
            canvas.draw_atlas(
                &atlas,
                &xforms,
                &rects,
                Some(&quad_colors[..]),
                *mode,
                SamplingOptions::default(),
                None,
                Some(&paint),
            );
            canvas.restore();
        }
    }
}

// Port of: gm/drawatlascolor.cpp#L158 (chrome/m156)
crate::def_gm!(DrawAtlasColorsGM, DrawAtlasColorsGm);
