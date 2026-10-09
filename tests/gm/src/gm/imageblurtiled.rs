// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imageblurtiled.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const WIDTH: i32 = 640;
const HEIGHT: i32 = 480;

// Port of: gm/imageblurtiled.cpp#L12-L51 (chrome/m156), ImageBlurTiledGM
struct ImageBlurTiledGm {
    sigma_x: f32,
    sigma_y: f32,
}

impl GM for ImageBlurTiledGm {
    fn name(&self) -> String {
        "imageblurtiled".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/imageblurtiled.cpp#L21-L47 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_image_filter(blur(
            self.sigma_x,
            self.sigma_y,
            TileMode::Decal,
            None,
            None,
        ));
        let tile_size = int_to_scalar(128);
        let bounds = canvas
            .local_clip_bounds()
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        let mut y = bounds.top();
        while y < bounds.bottom() {
            let mut x = bounds.left();
            while x < bounds.right() {
                canvas.save();
                canvas.clip_rect(Rect::from_xywh(x, y, tile_size, tile_size), None, None);
                canvas.save_layer(&SaveLayerRec::default().paint(&paint));
                let text = ["The quick", "brown fox", "jumped over", "the lazy dog."];
                let font = Font::from_size(default_portable_typeface(), 100.0);
                let mut pos_y: i32 = 0;
                for line in text {
                    pos_y += 100;
                    canvas.draw_str(line, (0.0, int_to_scalar(pos_y)), &font, &Paint::default());
                }
                canvas.restore();
                canvas.restore();
                x += tile_size;
            }
            y += tile_size;
        }
    }
}

// Port of: gm/imageblurtiled.cpp#L53 (chrome/m156), DEF_GM(return new ImageBlurTiledGM(3.0f, 3.0f);)
crate::def_gm!(
    ImageBlurTiledGM_3_3 = "ImageBlurTiledGM(3.0f, 3.0f)",
    ImageBlurTiledGm {
        sigma_x: 3.0,
        sigma_y: 3.0,
    }
);
