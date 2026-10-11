// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/complexclip_blur_tiled.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{int_to_scalar, make_surface};
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;

const WIDTH: i32 = 512;
const HEIGHT: i32 = 512;

// Port of: gm/complexclip_blur_tiled.cpp#L24-L65 (chrome/m156)
struct ComplexClipBlurTiledGm;

impl GM for ComplexClipBlurTiledGm {
    fn name(&self) -> String {
        "complexclip_blur_tiled".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/complexclip_blur_tiled.cpp#L30-L62 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut blur_paint = Paint::default();
        blur_paint.set_image_filter(blur(5.0, 5.0, TileMode::Decal, None, None));

        let tile_size = 128.0_f32;
        let bounds = canvas.local_clip_bounds().unwrap_or_default();
        // SkScalarCeilToInt(tileSize)
        let ts = 128;
        let info = ImageInfo::new_n32_premul((ts, ts), None);
        let mut tile_surface = make_surface(canvas, &info, None).expect("a tile surface");

        let mut y = bounds.top();
        while y < bounds.bottom() {
            let mut x = bounds.left();
            while x < bounds.right() {
                {
                    let tile_canvas = tile_surface.canvas();
                    tile_canvas.save();
                    tile_canvas.clear(Color::TRANSPARENT);
                    tile_canvas.translate((-x, -y));
                    let rect = Rect::from_wh(int_to_scalar(WIDTH), int_to_scalar(HEIGHT));
                    tile_canvas
                        .save_layer(&SaveLayerRec::default().bounds(&rect).paint(&blur_paint));
                    let rrect = RRect::new_rect_xy(rect.with_inset((20.0, 20.0)), 25.0, 25.0);
                    tile_canvas.clip_rrect(&rrect, ClipOp::Difference, true);
                    let paint = Paint::default();
                    tile_canvas.draw_rect(rect, &paint);
                    tile_canvas.restore();
                    tile_canvas.restore();
                }
                if let Some(snapshot) = tile_surface.image_snapshot() {
                    canvas.draw_image(&snapshot, (x, y), None);
                }
                x += tile_size;
            }
            y += tile_size;
        }
    }
}

// Port of: gm/complexclip_blur_tiled.cpp#L69 (chrome/m156)
crate::def_gm!(ComplexClipBlurTiledGM, ComplexClipBlurTiledGm);
