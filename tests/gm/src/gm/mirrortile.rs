// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/mirrortile.cpp (chrome/m156)

#![allow(clippy::similar_names)] // imgx/imgy, pmx/pmy: the C++ names

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/mirrortile.cpp#L20-L86 (chrome/m156)
crate::def_simple_gm_can_fail!(mirror_tile, canvas, error_msg, 140, 370, {
    // We don't run this test on the GPU because we're at the driver/hw's mercy for how this
    // is handled. We also don't test this on recording or vector canvases.
    if canvas.peek_pixels().is_none() {
        *error_msg = "Test only works with canvases backed by CPU pixels".to_string();
        return DrawResult::Skip;
    }

    let colors: [u32; 3] = [0xFFFF_0000, 0xFF00_FF00, 0xFF00_00FF];
    let color_bytes: Vec<u8> = colors.iter().flat_map(|c| c.to_ne_bytes()).collect();
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // std::size(colors)
    let n = colors.len() as i32;
    let pmx = Pixmap::new_readonly(
        &ImageInfo::new((n, 1), ColorType::RGBA8888, AlphaType::Premul, None),
        &color_bytes,
        color_bytes.len(),
    )
    .expect("a pixmap");
    let imgx = images::raster_from_pixmap_copy(&pmx).expect("an image");

    let pmy = Pixmap::new_readonly(
        &ImageInfo::new((1, n), ColorType::RGBA8888, AlphaType::Premul, None),
        &color_bytes,
        4,
    )
    .expect("a pixmap");
    let imgy = images::raster_from_pixmap_copy(&pmy).expect("an image");

    // We draw offscreen and then zoom that up to make the result clear.
    let mut surf = canvas
        .new_surface(&canvas.image_info().with_dimensions((80, 80)), None)
        .expect("a surface");
    {
        let c = surf.canvas();
        c.clear(Color::WHITE);

        #[allow(clippy::cast_precision_loss)] // imgx->width() and friends as SkScalar
        let (xw, yh) = (imgx.width() as f32, imgy.height() as f32);
        for offset in [false, true] {
            for fm in [FilterMode::Nearest, FilterMode::Linear] {
                let mut paint = Paint::default();

                // Draw single row image with mirror tiling in x and clamped in y.
                paint.set_shader(imgx.to_shader(
                    (TileMode::Mirror, TileMode::Clamp),
                    SamplingOptions::from(fm),
                    None,
                ));
                c.save();
                c.translate((xw, 0.0));
                if offset {
                    c.translate((0.5, 0.0));
                }
                c.draw_rect(Rect::from_xywh(-xw, 0.0, 3.0 * xw, 5.0), &paint);
                c.restore();

                // Draw single column image with mirror tiling in y and clamped in x.
                paint.set_shader(imgy.to_shader(
                    (TileMode::Clamp, TileMode::Mirror),
                    SamplingOptions::from(fm),
                    None,
                ));
                c.save();
                c.translate((3.0 * xw + 3.0, yh));
                if offset {
                    c.translate((0.0, 0.5));
                }
                c.draw_rect(Rect::from_xywh(0.0, -yh, 5.0, 3.0 * yh), &paint);
                c.restore();

                c.translate((0.0, 3.0 * yh + 3.0));
            }
        }
    }

    canvas.scale((8.0, 8.0));
    canvas.draw_image(surf.image_snapshot().expect("a snapshot"), (0.0, 0.0), None);
    DrawResult::Ok
});
