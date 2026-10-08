// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/tinybitmap.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/tinybitmap.cpp#L21-L37 (chrome/m156)
struct TinyBitmapGm;

impl GM for TinyBitmapGm {
    // `onOnceBeforeDraw`: `setBGColor(0xFFDDDDDD)`.
    fn bg_color(&self) -> Color {
        Color::new(0xFFDD_DDDD)
    }

    fn name(&self) -> String {
        "tinybitmap".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(100, 100)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut bm = Bitmap::new();
        bm.alloc_n32_pixels((1, 1), None);
        bm.set_addr32(0, 0, pack_argb32(0x80, 0x80, 0, 0));
        let mut paint = Paint::default();
        paint.set_alpha_f(0.5);
        paint.set_shader(bm.to_shader(
            (TileMode::Repeat, TileMode::Mirror),
            SamplingOptions::default(),
            None,
        ));
        canvas.draw_paint(&paint);
    }
}

// Port of: gm/tinybitmap.cpp#L39 (chrome/m156)
crate::def_gm!(TinyBitmapGM, TinyBitmapGm);
