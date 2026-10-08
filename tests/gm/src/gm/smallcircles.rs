// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/smallcircles.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

// Port of: gm/smallcircles.cpp#L13-L18 (chrome/m156)
fn draw_small_circle(canvas: &Canvas, p: &Paint, radius: f32) {
    const USE_CENTER: bool = false;
    const DEGREES: f32 = 360.0;
    let oval = Rect::new(-radius, -radius, radius, radius);
    canvas.draw_arc(oval, 0.0, DEGREES, USE_CENTER, p);
}

// Port of: gm/smallcircles.cpp#L20-L50 (chrome/m156)
crate::def_simple_gm!(smallcircles, canvas, 425, 425, {
    let info = ImageInfo::new_n32_premul((100, 100), None);
    let mut surface = canvas
        .new_surface(&info, None)
        .or_else(|| surfaces::raster(&info, None, None))
        .expect("a surface");
    {
        let canv = surface.canvas();

        canv.translate((5.0, 5.0));
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_style(Style::Fill);

        // Draw circles
        for _i in 0..11 {
            canv.save();
            for _j in 0..11 {
                draw_small_circle(canv, &p, 0.8);
                canv.translate((5.1, 0.0));
            }
            canv.restore();
            canv.translate((0.0, 5.1));
        }
    }

    // Scale up image of small circles
    canvas.scale((7.0, 7.0));
    let img = surface.image_snapshot().expect("a snapshot");
    canvas.draw_image(&img, (0.0, 0.0), None);
});
