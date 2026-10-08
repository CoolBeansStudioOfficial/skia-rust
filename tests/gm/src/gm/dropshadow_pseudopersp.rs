// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dropshadowimagefilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::m44::M44;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_effects::image_filters::drop_shadow;

// Port of: gm/dropshadowimagefilter.cpp#L140-L182 (chrome/m156)
crate::def_simple_gm!(dropshadow_pseudopersp, canvas, 155, 155, {
    canvas.clear(Color::LIGHT_GRAY);
    canvas.concat_44(&M44::new(
        0.5, 0.0, 0.0, -75.0, 0.0, 0.5, 0.0, -30.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ));
    // This 4x4 matrix technically has perspective, but it only impacts the Z values and the
    // projection doesn't appear to have any distortion. However, the projected coordinates have
    // Z values very different from 0. When inversing the device bounds with an assumed Z=0, the
    // layer bounds end up empty. This GM ensures layer mapping calculations don't discard it.
    canvas.concat_44(&M44::new(
        1360.0, 0.0, 275.4, 294_100.0, 0.0, 1360.0, 489.6, 98_344.0, 0.0, 0.0, -0.51, -2180.67,
        0.0, 0.0, 0.51, 2181.67,
    ));

    let layer_bounds = Rect::new(42.5, 42.5, 457.5, 457.5);
    let mut layer_paint = Paint::default();
    layer_paint.set_image_filter(drop_shadow(
        (30.0, 30.0),
        (12.0, 12.0),
        Color4f::new(0.14902, 0.215_686, 0.329_412, 0.666_667),
        None,
        None,
        None,
    ));
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&layer_bounds)
            .paint(&layer_paint),
    );

    let rrect = RRect::new_rect_xy(Rect::new(-250.0, -250.0, 250.0, 250.0), 45.0, 45.0);
    let mut rrect_paint = Paint::default();
    rrect_paint.set_color(Color::WHITE);
    rrect_paint.set_anti_alias(true);

    canvas.concat_44(&M44::new(
        0.83, 0.0, 0.0, 250.0, 0.0, 0.83, 0.0, 250.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ));
    canvas.draw_rrect(rrect, &rrect_paint);
    canvas.restore();

    canvas.concat_44(&M44::new(
        0.83, 0.0, 0.0, 250.0, 0.0, 0.83, 0.0, 250.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ));

    rrect_paint.set_color(Color::BLACK);
    rrect_paint.set_style(Style::Stroke);
    canvas.draw_rrect(rrect, &rrect_paint);
});
