// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug1719.cpp (chrome/m156)

// The C++ float literals (e.g. 657.58173f) are rounded to f32 here, as they are in the C++.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

// This test exercises skbug.com/40032817. An anti-aliased blurred path is rendered through a soft
// clip. On the GPU a scratch texture was used to hold the original path mask as well as the blurred
// path result. The same texture is then incorrectly used to generate the soft clip mask for the
// draw. Thus the same texture is used for both the blur mask and soft mask in a single draw.
//
// The correct image should look like a thin stroked round rect.

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::color_filters;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;

// Port of: gm/skbug1719.cpp (chrome/m156), the round-rect clip path
fn clip_path() -> Path {
    PathBuilder::new()
        .move_to((832.0, 654.0))
        .line_to((1034.0, 654.0))
        .cubic_to((1038.4183, 654.0), (1042.0, 657.58173), (1042.0, 662.0))
        .line_to((1042.0, 724.0))
        .cubic_to((1042.0, 728.41827), (1038.4183, 732.0), (1034.0, 732.0))
        .line_to((832.0, 732.0))
        .cubic_to((827.58173, 732.0), (824.0, 728.41827), (824.0, 724.0))
        .line_to((824.0, 662.0))
        .cubic_to((824.0, 657.58173), (827.58173, 654.0), (832.0, 654.0))
        .close()
        .detach()
}

// This is a round rect nested inside a rect.
// Port of: gm/skbug1719.cpp (chrome/m156), the even-odd draw path
fn draw_path() -> Path {
    PathBuilder::new_with_fill_type(PathFillType::EvenOdd)
        .move_to((823.0, 653.0))
        .line_to((1043.0, 653.0))
        .line_to((1043.0, 733.0))
        .line_to((823.0, 733.0))
        .line_to((823.0, 653.0))
        .close()
        .move_to((832.0, 654.0))
        .line_to((1034.0, 654.0))
        .cubic_to((1038.4183, 654.0), (1042.0, 657.58173), (1042.0, 662.0))
        .line_to((1042.0, 724.0))
        .cubic_to((1042.0, 728.41827), (1038.4183, 732.0), (1034.0, 732.0))
        .line_to((832.0, 732.0))
        .cubic_to((827.58173, 732.0), (824.0, 728.41827), (824.0, 724.0))
        .line_to((824.0, 662.0))
        .cubic_to((824.0, 657.58173), (827.58173, 654.0), (832.0, 654.0))
        .close()
        .detach()
}

// Port of: gm/skbug1719.cpp (chrome/m156), DEF_SIMPLE_GM_BG(skbug1719, canvas, 300, 100, 0xFF303030)
crate::def_simple_gm_bg!(skbug1719, canvas, 300, 100, Color::new(0xFF30_3030), {
    canvas.translate((-800.0, -650.0));

    // The data is lifted from an SKP that exhibited the bug.
    let clip = clip_path();
    let draw = draw_path();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::new(0xFF00_0000));
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 0.78867501, None));
    paint.set_color_filter(color_filters::blend_color(
        Color::new(0xBFFF_FFFF),
        BlendMode::SrcIn,
    ));

    canvas.clip_path(&clip, None, true);
    canvas.draw_path(&draw, &paint);
});
