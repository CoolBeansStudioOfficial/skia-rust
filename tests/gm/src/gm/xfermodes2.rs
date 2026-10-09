// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/xfermodes2.cpp (chrome/m156)

// The int-to-scalar casts of small constants and the byte casts of packed channels mirror the C++
// arithmetic of the GM.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
// Single-letter and similar names mirror the C++ GM (w, h, x, y; rect, rrect).
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/xfermodes2.cpp#L49-L51 (chrome/m156), enum { kShift = 2, kSize = 256 >> kShift }
const K_SHIFT: i32 = 2;
const K_SIZE: i32 = 256 >> K_SHIFT;

// Port of: gm/xfermodes2.cpp (chrome/m156), class Xfermodes2GM
#[derive(Debug)]
pub struct Xfermodes2Gm {
    grayscale: bool,
    bg: Option<Shader>,
    src: Option<Shader>,
    dst: Option<Shader>,
}

impl Xfermodes2Gm {
    // Port of: gm/xfermodes2.cpp (chrome/m156), Xfermodes2GM(bool)
    #[must_use]
    pub fn new(grayscale: bool) -> Self {
        Self {
            grayscale,
            bg: None,
            src: None,
            dst: None,
        }
    }
}

impl GM for Xfermodes2Gm {
    // Port of: gm/xfermodes2.cpp (chrome/m156), getName
    fn name(&self) -> String {
        if self.grayscale {
            "xfermodes2_gray".to_owned()
        } else {
            "xfermodes2".to_owned()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(455, 475)
    }

    // Port of: gm/xfermodes2.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((10.0, 20.0));

        let w = K_SIZE as scalar;
        let h = K_SIZE as scalar;

        let font = default_portable_font();

        let k_w = 6;

        let mut x: scalar = 0.0;
        let mut y: scalar = 0.0;
        for (m, &mode) in BlendMode::VALUES.iter().enumerate() {
            canvas.save();

            canvas.translate((x, y));
            let mut p = Paint::default();
            p.set_anti_alias(false);
            p.set_style(Style::Fill);
            p.set_shader(self.bg.clone());
            let mut r = Rect::from_wh(w, h);
            canvas.draw_rect(r, &p);

            canvas.save_layer(&skia_rust_core::canvas::SaveLayerRec::default().bounds(&r));

            p.set_shader(self.dst.clone());
            canvas.draw_rect(r, &p);
            p.set_shader(self.src.clone());
            p.set_blend_mode(mode);
            canvas.draw_rect(r, &p);

            canvas.restore();

            r.inset((-0.5, -0.5));
            p.set_style(Style::Stroke);
            p.set_shader(None);
            p.set_blend_mode(BlendMode::SrcOver);
            canvas.draw_rect(r, &p);

            canvas.restore();

            text_utils::draw_string(
                canvas,
                mode.name(),
                x + w / 2.0,
                y - font.size() / 2.0,
                &font,
                &Paint::default(),
                Align::Center,
            );
            x += w + 10.0;
            if m as i32 % k_w == k_w - 1 {
                x = 0.0;
                y += h + 30.0;
            }
        }
    }

    // Port of: gm/xfermodes2.cpp (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let check_data = [
            pack_argb32(0xFF, 0x42, 0x41, 0x42),
            pack_argb32(0xFF, 0xD6, 0xD3, 0xD6),
            pack_argb32(0xFF, 0xD6, 0xD3, 0xD6),
            pack_argb32(0xFF, 0x42, 0x41, 0x42),
        ];
        let mut bg = Bitmap::new();
        bg.alloc_n32_pixels((2, 2), true);
        for (i, &v) in check_data.iter().enumerate() {
            bg.set_addr32((i % 2) as i32, (i / 2) as i32, v);
        }

        let lm = Matrix::scale((16.0, 16.0));
        self.bg = bg.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            &lm,
        );

        let mut src_bmp = Bitmap::new();
        src_bmp.alloc_n32_pixels((K_SIZE, K_SIZE), None);
        for y in 0..K_SIZE {
            let c = y * (1 << K_SHIFT);
            let row_color = if self.grayscale {
                pack_argb32(c as u32, c as u32, c as u32, c as u32)
            } else {
                pack_argb32(c as u32, c as u32, 0, (c / 2) as u32)
            };
            for x in 0..K_SIZE {
                src_bmp.set_addr32(x, y, row_color);
            }
        }
        self.src = src_bmp.to_shader(None, SamplingOptions::default(), None);

        let mut dst_bmp = Bitmap::new();
        dst_bmp.alloc_n32_pixels((K_SIZE, K_SIZE), None);
        for x in 0..K_SIZE {
            let c = x * (1 << K_SHIFT);
            let col_color = if self.grayscale {
                pack_argb32(c as u32, c as u32, c as u32, c as u32)
            } else {
                pack_argb32(c as u32, 0, c as u32, (c / 2) as u32)
            };
            for y in 0..K_SIZE {
                dst_bmp.set_addr32(x, y, col_color);
            }
        }
        self.dst = dst_bmp.to_shader(None, SamplingOptions::default(), None);
    }
}

// Port of: gm/xfermodes2.cpp#L146-L147 (chrome/m156), DEF_GM( return new Xfermodes2GM(false); )
crate::def_gm!(
    Xfermodes2GM_false = "Xfermodes2GM(false)",
    Xfermodes2Gm::new(false)
);
// Port of: gm/xfermodes2.cpp#L146-L147 (chrome/m156), DEF_GM( return new Xfermodes2GM(true); )
crate::def_gm!(
    Xfermodes2GM_true = "Xfermodes2GM(true)",
    Xfermodes2Gm::new(true)
);
