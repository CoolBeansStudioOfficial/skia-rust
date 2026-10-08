// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurroundrect.cpp (chrome/m156)
//
// skia-rust: `SimpleBlurRoundRectGM` needs a two point conical gradient shader (Phase 3) and is
// not ported yet; see the manifest.

use crate::prelude::*;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// From crbug.com/1138810
// Port of: gm/blurroundrect.cpp#L100-L114 (chrome/m156)
crate::def_simple_gm!(blur_large_rrects, canvas, 300, 300, {
    let mut paint = Paint::default();
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 20.0, None));

    let long_rect = Rect::new(5.0, -20000.0, 240.0, 25.0);
    let rrect = RRect::new_rect_xy(long_rect, 40.0, 40.0);
    for i in 0..4 {
        let color = Color4f::new(
            if (i & 1) != 0 { 1.0 } else { 0.0 },
            if (i & 2) != 0 { 1.0 } else { 0.0 },
            if i < 2 { 1.0 } else { 0.0 },
            1.0,
        );
        paint.set_color4f(color, None);
        canvas.draw_rrect(rrect, &paint);
        canvas.rotate(90.0, Some(Point::new(150.0, 150.0)));
    }
});
