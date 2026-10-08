// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bug6783.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::surfaces;

// This GM reproduces skbug.com/40037998, which demonstrated a bug in repeat and mirror
// image sampling tiling modes as implemented in software.  We want to tile to
// [0,limit), and the old incorrect logic was:
//
//    limit = ulp_before(limit)
//    val = val - floor(val/limit)*limit    (This is repeat; mirror is similar.)
//
// while the correct logic is more like:
//
//    val = val - floor(val/limit)*limit
//    val = min(val, ulp_before(limit))
//
// You would see ugly jaggies on the blue/yellow edge near the bottom left if
// the bug were still present.  All stripes should now look roughly the same.

// Port of: gm/bug6783.cpp#L37-L60 (chrome/m156)
crate::def_simple_gm!(bug6783, canvas, 500, 500, {
    let mut surface = surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None)
        .expect("a surface");

    let mut p = Paint::default();
    p.set_color(Color::YELLOW);
    surface.canvas().draw_paint(&p);
    p.set_color(Color::BLUE);
    surface.canvas().draw_rect(Rect::from_wh(50.0, 100.0), &p);

    let img = surface.image_snapshot().expect("a snapshot");

    let mut m = Matrix::translate((25.0, 214.0)) * Matrix::scale((2.0, 2.0));
    m.pre_skew((0.5, 0.5), None);

    // The bug was present at all filter levels, but you might not notice it at nearest.
    let sampling = SamplingOptions::from(FilterMode::Linear);

    // It's only important to repeat or mirror in x to show off the bug.
    p.set_shader(img.to_shader((TileMode::Repeat, TileMode::Clamp), sampling, &m));
    canvas.draw_paint(&p);
});
