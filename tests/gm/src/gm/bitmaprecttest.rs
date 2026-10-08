// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmaprecttest.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::image::Image;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/bitmaprecttest.cpp#L18-L31 (chrome/m156)
fn make_bm() -> Option<Image> {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((60, 60), None);
    bm.erase_color(Color::new(0));

    {
        let canvas = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
        let mut paint = Paint::default();
        canvas.draw_path(
            &Path::polygon(
                &[
                    Point::new(6.0, 6.0),
                    Point::new(6.0, 54.0),
                    Point::new(30.0, 54.0),
                ],
                false,
                None,
                None,
            ),
            &paint,
        );

        paint.set_style(Style::Stroke);
        canvas.draw_rect(Rect::new(0.5, 0.5, 59.5, 59.5), &paint);
    }
    bm.as_image()
}

// This creates a close, but imperfect concatenation of
//      scaling the image up by its dst-rect
//      scaling the image down by the matrix' scale
//  The bug was that for cases like this, we were incorrectly trying to take a
//  fast-path in the bitmapshader, but ended up drawing the last col of pixels
//  twice. The fix resulted in (a) not taking the fast-path, but (b) drawing
//  the image correctly.
//
// Port of: gm/bitmaprecttest.cpp#L33-L52 (chrome/m156)
crate::def_simple_gm!(bitmaprecttest, canvas, 320, 240, {
    let image = make_bm().expect("an image");

    canvas.draw_image(&image, (150.0, 45.0), None);

    let scale: scalar = 0.472_56;
    canvas.save();
    canvas.scale((scale, scale));
    canvas.draw_image_rect_with_sampling_options(
        &image,
        None,
        Rect::from_xywh(100.0, 100.0, 128.0, 128.0),
        SamplingOptions::default(),
        &Paint::default(),
    );
    canvas.restore();

    canvas.scale((-1.0, 1.0));
    canvas.draw_image(&image, (-310.0, 45.0), None);
});
