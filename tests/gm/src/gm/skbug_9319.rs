// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_9319.cpp (chrome/m156)

use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Illustrates a bug where the outer portion of the GPU rect blur was too dark with a small sigma.
// Port of: gm/skbug_9319.cpp#L14-L36 (chrome/m156), DEF_SIMPLE_GM(skbug_9319)
crate::def_simple_gm!(skbug_9319, canvas, 256, 512, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 0.5, None));

    let r = Rect::from_xywh(10.0, 10.0, 100.0, 100.0);

    {
        canvas.save();
        // Clip out interior so that the outer portion stands out.
        canvas.clip_rect(r, ClipOp::Difference, None);
        canvas.draw_rect(r, &p);
        canvas.restore();
    }

    canvas.translate((0.0, 120.0));

    // RRect for comparison.
    let rr = RRect::new_rect_xy(r, 0.1, 0.1);
    {
        canvas.save();
        canvas.clip_rrect(rr, ClipOp::Difference, None);
        canvas.draw_rrect(rr, &p);
        canvas.restore();
    }
});
