// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/clipdrawdraw.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::Region;

// This GM exercises the use case found in crbug.com/423834.
// The following pattern:
//    save();
//    clipRect(rect, noAA);
//    drawRect(bigRect, noAA);
//    restore();
//
//    drawRect(rect, noAA);
// can leave 1 pixel wide remnants of the first rect.
// Port of: gm/clipdrawdraw.cpp#L22-L41 (chrome/m156)
fn draw(canvas: &Canvas, rect: &Rect) {
    let mut p = Paint::default();
    p.set_anti_alias(false);

    let big_rect = Rect::from_wh(600.0, 600.0);

    canvas.save();
    // draw a black rect through the clip
    canvas.save();
    canvas.clip_rect(rect, None, None);
    canvas.draw_rect(big_rect, &p);
    canvas.restore();

    // now draw the white rect on top
    p.set_color(Color::WHITE);
    canvas.draw_rect(rect, &p);
    canvas.restore();
}

// Port of: gm/clipdrawdraw.cpp#L43-L54 (chrome/m156)
crate::def_simple_gm_bg!(clipdrawdraw, canvas, 512, 512, Color::from(0xFFCC_CCCC), {
    // Vertical remnant
    let rect1 = Rect::from_ltrb(136.5, 137.5, 338.5, 293.5);

    // Horizontal remnant
    // 179.488 rounds the right way (i.e., 179), 179.499 rounds the wrong way (i.e., 180)
    let rect2 = Rect::from_ltrb(207.5, 179.499, 530.5, 429.5);

    draw(canvas, &rect1);
    draw(canvas, &rect2);
});

// Port of: gm/clipdrawdraw.cpp#L58-L72 (chrome/m156)
crate::def_simple_gm!(clip_region, canvas, 256, 256, {
    let rgn = Region::from_rect(IRect::new(10, 10, 100, 100));

    canvas.save();
    canvas.clip_region(&rgn, None);
    canvas.draw_color(Color::RED, None);
    canvas.restore();

    let bounds = Rect::new(30.0, 30.0, 80.0, 80.0);
    canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
    canvas.clip_region(&rgn, None);
    canvas.draw_color(Color::BLUE, None);
    canvas.restore();
});
