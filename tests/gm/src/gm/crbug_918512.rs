// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_918512.cpp (chrome/m156)

// PDF backend should produce correct results.
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{AutoCanvasRestore, SaveLayerRec};
use skia_rust_core::color::{Color, colors};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_effects::luma_color_filter;

// Port of: gm/crbug_918512.cpp#L11-L30 (chrome/m156), DEF_SIMPLE_GM(crbug_918512)
crate::def_simple_gm!(crbug_918512, canvas, 256, 256, {
    canvas.draw_color(Color::YELLOW, None);
    {
        let _acr1 = AutoCanvasRestore::guard(canvas, false);
        canvas.save_layer(&SaveLayerRec::default());
        canvas.draw_color(Color::CYAN, None);
        {
            let _acr2 = AutoCanvasRestore::guard(canvas, false);
            let mut luma_filter = Paint::default();
            luma_filter.set_blend_mode(BlendMode::DstIn);
            luma_filter.set_color_filter(luma_color_filter::make());
            canvas.save_layer(&SaveLayerRec::default().paint(&luma_filter));
            canvas.draw_color(colors::TRANSPARENT, None);
            let mut paint = Paint::default();
            paint.set_color(Color::GRAY);
            canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 128.0, 256.0), &paint);
        }
    }
});
