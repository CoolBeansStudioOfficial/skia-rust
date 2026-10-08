// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurs.cpp (chrome/m156)
//
// skia-rust: the GMs that need text (`blurs`), images (`BlurDrawImage`) or the blur image filter
// (`BlurBigSigma`, `BlurSmallSigma`, `TiledBlurBigSigma`) are not ported yet; see the manifest.

use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_round_to_scalar;

// exercise a special-case of blurs, which is two nested rects. These are drawn specially,
// and possibly cached.
//
// in particular, we want to notice that the 2nd rect draws slightly differently, since it
// is translated a fractional amount.
//
// Port of: gm/blurs.cpp#L90-L111 (chrome/m156)
crate::def_simple_gm!(blur2rects, canvas, 700, 500, {
    let mut paint = Paint::default();

    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 2.3, None));

    let outer = Rect::from_xywh(10.125, 10.125, 100.125, 100.0);
    let inner = Rect::from_xywh(20.25, 20.125, 80.0, 80.0);
    let path = PathBuilder::new()
        .add_rect(outer, PathDirection::CW, None)
        .add_rect(inner, PathDirection::CCW, None)
        .detach();

    canvas.draw_path(&path, &paint);
    // important to translate by a factional amount to exercise a different "phase"
    // of the same path w.r.t. the pixel grid
    let dx = scalar_round_to_scalar(path.bounds().width()) + 14.0 + 0.25;
    canvas.translate((dx, 0.0));
    canvas.draw_path(&path, &paint);
});

// Port of: gm/blurs.cpp#L113-L133 (chrome/m156)
crate::def_simple_gm!(blur2rectsnonninepatch, canvas, 700, 500, {
    let mut paint = Paint::default();
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 4.3, None));

    let outer = Rect::from_xywh(10.0, 110.0, 100.0, 100.0);
    let inner = Rect::from_xywh(50.0, 150.0, 10.0, 10.0);
    let path = PathBuilder::new()
        .add_rect(outer, PathDirection::CW, None)
        .add_rect(inner, PathDirection::CW, None)
        .detach();
    canvas.draw_path(&path, &paint);

    let dx = scalar_round_to_scalar(path.bounds().width()) + 40.0 + 0.25;
    canvas.translate((dx, 0.0));
    canvas.draw_path(&path, &paint);

    // Translate to outside of clip bounds.
    canvas.translate((-dx, 0.0));
    canvas.translate((-30.0, -150.0));
    canvas.draw_path(&path, &paint);
});
