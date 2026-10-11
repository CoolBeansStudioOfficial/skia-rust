// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_905548.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{arithmetic, blend, blur, erode, image_sampled};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

// Port of: gm/crbug_905548.cpp#L8-L41 (chrome/m156), crbug_905548
crate::def_simple_gm!(crbug_905548, canvas, 100, 200, {
    let info = ImageInfo::new_n32_premul((100, 100), None);
    let mut surface = canvas
        .new_surface(&info, None)
        .or_else(|| surfaces::raster(&info, None, None))
        .expect("a surface");
    surface.canvas().clear(Color::TRANSPARENT);
    surface
        .canvas()
        .draw_circle((50.0, 50.0), 45.0, &Paint::default());
    let image_source = image_sampled(
        surface.image_snapshot(),
        SamplingOptions::from(FilterMode::Nearest),
    );
    let blurred = blur(15.0, 15.0, TileMode::Decal, image_source.clone(), None);
    let eroded = erode((0.0, 0.0), blurred.clone(), None);
    let blended = blend(
        BlendMode::DstOut,
        eroded.clone(),
        image_source.clone(),
        None,
    );
    let mut paint = Paint::default();
    paint.set_image_filter(blended);
    canvas.draw_rect(Rect::from_wh(100.0, 100.0), &paint);

    let mult = arithmetic(1.0, 0.0, 0.0, 0.0, false, eroded, image_source, None);
    paint.set_image_filter(mult);
    canvas.translate((0.0, 100.0));
    canvas.draw_rect(Rect::from_wh(100.0, 100.0), &paint);
});
