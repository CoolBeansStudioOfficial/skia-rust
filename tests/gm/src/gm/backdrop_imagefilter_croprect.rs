// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/backdrop_imagefilter_croprect.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::canvas::{SaveLayerFlags, SaveLayerRec};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, color_filter};

// Port of: gm/backdrop_imagefilter_croprect.cpp#L15-L70 (chrome/m156), draw_backdrop_filter_gm
fn draw_backdrop_filter_gm(
    canvas: &Canvas,
    outset_x: f32,
    outset_y: f32,
    factory: fn(&IRect) -> Option<ImageFilter>,
) {
    // CTM translates to (150, 150)
    let mut origin = Point::new(150.0, 150.0);
    // The save layer specified after the CTM has negative coordinates, but
    // means that (100, 100) to (500, 250) in device-space will be saved
    let clip = Rect::from_xywh(-50.0, -50.0, 400.0, 150.0);
    // The image-filter crop relative to the CTM, which will map to
    // (200, 160) to (400, 190) in device space, or (100, 600) to (300, 90) in
    // the layer's image space.
    let crop_in_local = Rect::new(50.0, 10.0, 250.0, 40.0);

    let mut outset = crop_in_local;
    outset.outset((outset_x, outset_y));
    let crop_rect: IRect = RoundOut::<IRect>::round_out(&outset);
    let image_filter = factory(&crop_rect);

    let mut p = Paint::default();
    for i in 0..2 {
        canvas.save();
        canvas.translate((origin.x, origin.y));

        canvas.clip_rect(clip, None, None);

        if i == 0 {
            // Primary save layer mode, so save layer before drawing the content
            let mut imf_paint = Paint::default();
            imf_paint.set_image_filter(image_filter.clone());
            canvas.save_layer(&SaveLayerRec::default().paint(&imf_paint));
        }
        // else backdrop mode, so the content is drawn first

        // Fill the clip with one color (cyan for i == 0 (inverse = red), and
        // magenta for i == 1 (inverse = green))
        p.set_color(if i == 0 { Color::CYAN } else { Color::MAGENTA });
        canvas.draw_paint(&p);

        // Then an inner rectangle with a color meant to be inverted by the image filter
        p.set_color(if i == 0 { Color::RED } else { Color::GREEN });
        canvas.draw_rect(crop_in_local, &p);

        if i == 1 {
            // Backdrop mode, so save a layer using the image filter as the backdrop to filter
            // content on initialization.
            let mut rec = SaveLayerRec::default().flags(SaveLayerFlags::INIT_WITH_PREVIOUS);
            if let Some(filter) = &image_filter {
                rec = rec.backdrop(filter);
            }
            canvas.save_layer(&rec);
        }

        // Restore the saved layer (either a main layer that was just drawn into and needs to be
        // filtered, or an "empty" layer initialized with the previously filtered backdrop)
        canvas.restore();

        // Move down
        canvas.restore();
        origin.y += 150.0;
    }
}

// Port of: gm/backdrop_imagefilter_croprect.cpp#L72-L77 (chrome/m156), make_invert_filter
fn make_invert_filter(crop: &IRect) -> Option<ImageFilter> {
    #[rustfmt::skip]
    let matrix = ColorMatrix::new(
        -1.0, 0.0, 0.0, 0.0, 1.0,
        0.0, -1.0, 0.0, 0.0, 1.0,
        0.0, 0.0, -1.0, 0.0, 1.0,
        0.0, 0.0, 0.0, 1.0, 0.0,
    );
    let cf = color_filters::matrix(&matrix, Clamp::Yes);
    color_filter(cf, None, Some(Rect::from_irect(crop)))
}

// Port of: gm/backdrop_imagefilter_croprect.cpp#L79-L83 (chrome/m156), make_blur_filter
fn make_blur_filter(crop: &IRect) -> Option<ImageFilter> {
    // Use different sigmas for x and y so rotated CTM is apparent
    blur(
        16.0,
        4.0,
        TileMode::Decal,
        None,
        Some(Rect::from_irect(crop)),
    )
}

// This draws correctly if there's a small cyan rectangle above a much larger magenta rectangle.
// There should be no red around the cyan rectangle and no green within the magenta rectangle.
// Port of: gm/backdrop_imagefilter_croprect.cpp#L85-L87 (chrome/m156)
crate::def_simple_gm!(backdrop_imagefilter_croprect, canvas, 600, 500, {
    draw_backdrop_filter_gm(canvas, 0.0, 0.0, make_invert_filter);
});

// This draws correctly if there's a blurred red rectangle inside a cyan rectangle, above a blurred
// green rectangle inside a larger magenta rectangle. All rectangles and the blur direction are
// consistently rotated.
// Port of: gm/backdrop_imagefilter_croprect.cpp#L89-L95 (chrome/m156)
crate::def_simple_gm!(backdrop_imagefilter_croprect_rotated, canvas, 600, 500, {
    canvas.translate((140.0, -180.0));
    canvas.rotate(30.0, None);
    draw_backdrop_filter_gm(canvas, 32.0, 32.0, make_blur_filter);
});

// This draws correctly if there's a blurred red rectangle inside a cyan rectangle, above a blurred
// green rectangle inside a larger magenta rectangle. All rectangles and the blur direction are
// under consistent perspective.
// NOTE: Currently renders incorrectly, see skbug.com/40040358
// Port of: gm/backdrop_imagefilter_croprect.cpp#L97-L105 (chrome/m156)
crate::def_simple_gm!(backdrop_imagefilter_croprect_persp, canvas, 600, 500, {
    let mut persp = Matrix::new_identity();
    persp.set_persp_y(0.001);
    persp.set_skew_x(8.0 / 25.0);
    canvas.concat(&persp);
    draw_backdrop_filter_gm(canvas, 32.0, 32.0, make_blur_filter);
});

// This draws correctly if there's a small cyan rectangle above a much larger magenta rectangle.
// There should be no red around the cyan rectangle and no green within the magenta rectangle, and
// everything should be 50% transparent.
// Port of: gm/backdrop_imagefilter_croprect.cpp#L107-L118 (chrome/m156)
crate::def_simple_gm!(backdrop_imagefilter_croprect_nested, canvas, 600, 500, {
    let mut p = Paint::default();
    p.set_alpha_f(0.5);
    // This ensures there is a non-root device on the stack with a non-zero origin.
    canvas.translate((15.0, 10.0));
    canvas.clip_rect(Rect::new(0.0, 0.0, 600.0, 500.0), None, None);

    canvas.save_layer(&SaveLayerRec::default().paint(&p));
    draw_backdrop_filter_gm(canvas, 0.0, 0.0, make_invert_filter);
    canvas.restore();
});

// Port of: gm/backdrop_imagefilter_croprect.cpp#L120-L150 (chrome/m156)
crate::def_simple_gm!(backdrop_layer_tilemode, canvas, 512, 128, {
    let draw_backdrop_tile_mode = |backdrop_tile_mode: TileMode| {
        canvas.save();
        // Restrict the canvas before starting a new layer to control its size.
        canvas.clip_rect(Rect::new(0.0, 0.0, 128.0, 128.0), None, None);
        // This layer will be the backdrop content, but without any additional effects, its size
        // will match the clip (128x128).
        canvas.save_layer(&SaveLayerRec::default());
        // Fill the layer with high frequency content (stripes of red and white)
        let mut y = 0;
        while y < 128 {
            let mut fill = Paint::default();
            fill.set_color(if y % 16 != 0 {
                Color::RED
            } else {
                Color::WHITE
            });
            let yf = int_to_scalar(y);
            canvas.draw_rect(Rect::new(0.0, yf, 128.0, yf + 8.0), &fill);
            y += 8;
        }
        // Perform a backdrop blur layer with the specified backdrop tile mode
        let blur_filter = blur(32.0, 32.0, TileMode::Decal, None, None);
        let mut rec = SaveLayerRec::default().backdrop_tile_mode(backdrop_tile_mode);
        if let Some(filter) = &blur_filter {
            rec = rec.backdrop(filter);
        }
        canvas.save_layer(&rec);
        canvas.restore();
        canvas.restore();
        canvas.restore();

        canvas.translate((128.0, 0.0));
    };

    draw_backdrop_tile_mode(TileMode::Clamp);
    draw_backdrop_tile_mode(TileMode::Decal);
    draw_backdrop_tile_mode(TileMode::Repeat);
    draw_backdrop_tile_mode(TileMode::Mirror);
});
