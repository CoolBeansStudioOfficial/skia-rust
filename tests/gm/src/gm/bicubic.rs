// Copyright 2020 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bicubic.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::surfaces;

// Port of: gm/bicubic.cpp#L15-L58 (chrome/m156)
crate::def_simple_gm!(bicubic, canvas, 300, 320, {
    canvas.clear(Color::BLACK);

    let g_samplings = [
        SamplingOptions::from(FilterMode::Nearest),
        SamplingOptions::from(FilterMode::Linear),
        SamplingOptions::from(CubicResampler::mitchell()),
    ];

    let make_img = || {
        let mut surf = surfaces::raster(&ImageInfo::new_n32_premul((7, 7), None), None, None)
            .expect("a surface");
        surf.canvas().draw_color(Color::BLACK, None);

        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        surf.canvas().draw_line((3.5, 0.0), (3.5, 8.0), &paint);
        surf.image_snapshot().expect("a snapshot")
    };

    let img = make_img();

    #[allow(clippy::cast_precision_loss)] // img->height() + 1.0f
    let step = img.height() as f32 + 1.0;
    canvas.scale((40.0, 8.0));
    for s in g_samplings {
        canvas.draw_image_with_sampling_options(&img, (0.0, 0.0), s, None);
        canvas.translate((0.0, step));
    }

    #[allow(clippy::cast_precision_loss)] // SkRect::MakeIWH
    let r = Rect::from_wh(img.width() as f32, img.height() as f32);
    let mut paint = Paint::default();

    let cubics = [CubicResampler::catmull_rom(), CubicResampler::mitchell()];
    for c in cubics {
        paint.set_shader(img.to_shader(
            (TileMode::Clamp, TileMode::Clamp),
            SamplingOptions::from(c),
            None,
        ));
        canvas.draw_rect(r, &paint);
        canvas.translate((0.0, step));
    }
});
