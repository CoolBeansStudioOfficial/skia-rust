// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/ducky_yuv_blend.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_possible_truncation)]

use crate::prelude::*;
use crate::tool_utils::{draw_checkerboard, get_resource_as_image};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};

// Modeled on the layout test css3/blending/background-blend-mode-image-image.html to reproduce
// skbug.com/40040948
//
// The second row of foreground images is the JPEG decoded to YUV planes on the GPU. On raster
// (this port) the C++ takes its `else` branch and draws the same JPEG again, so both rows here are
// the JPEG.
// Port of: gm/ducky_yuv_blend.cpp#L24-L84 (chrome/m156), ducky_yuv_blend
crate::def_simple_gm_can_fail!(ducky_yuv_blend, canvas, error_msg, 560, 1130, {
    let Some(ducky_bg) = get_resource_as_image("images/ducky.png") else {
        error_msg.clear();
        error_msg.push_str("Image(s) failed to load.");
        return DrawResult::Fail;
    };
    let Some(ducky_fg) = get_resource_as_image("images/ducky.jpg") else {
        error_msg.clear();
        error_msg.push_str("Image(s) failed to load.");
        return DrawResult::Fail;
    };
    // duckyFG[1] = duckyFG[0] on raster.
    let duck_fg = [ducky_fg.clone(), ducky_fg];

    const NUM_PER_ROW: i32 = 4;
    const PAD: f32 = 10.0;
    let dst_rect = Rect::from_wh(130.0, 130.0);
    let mut row_cnt = 0;

    // Moves to the next row, as the C++ `newRow` lambda does.
    let new_row = |row_cnt: &mut i32| {
        canvas.restore();
        canvas.translate((0.0, dst_rect.height() + PAD));
        canvas.save();
        *row_cnt = 0;
    };

    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest);
    canvas.translate((PAD, PAD));
    canvas.save();
    // SK_ColorDKGRAY and SK_ColorLTGRAY
    draw_checkerboard(
        canvas,
        Color::new(0xFF44_4444),
        Color::new(0xFFCC_CCCC),
        ((dst_rect.height() + PAD) / 5.0) as i32,
    );
    for fg in &duck_fg {
        // SkBlendMode::kLastCoeffMode + 1 ..< SkBlendMode::kLastMode
        for bm in (BlendMode::LAST_COEFF_MODE as i32 + 1)..(BlendMode::LAST_MODE as i32) {
            canvas.draw_image_rect_with_sampling_options(
                &ducky_bg,
                None,
                dst_rect,
                sampling,
                &Paint::default(),
            );
            let mut paint = Paint::default();
            paint.set_blend_mode(BlendMode::from_i32(bm).expect("blend mode"));
            canvas.draw_image_rect_with_sampling_options(fg, None, dst_rect, sampling, &paint);
            canvas.translate((dst_rect.width() + PAD, 0.0));
            row_cnt += 1;
            if row_cnt == NUM_PER_ROW {
                new_row(&mut row_cnt);
            }
        }
        // Force a new row between the two foreground images
        new_row(&mut row_cnt);
    }
    canvas.restore();
    DrawResult::Ok
});
