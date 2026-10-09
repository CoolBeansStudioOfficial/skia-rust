// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurignorexform.cpp (chrome/m156)
//
// `kBench_Mode` is never the mode of a DM draw here, so the overlay is always drawn.

// GM ports mirror the C++ integer and scalar casts and loop indices.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::similar_names
)]

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_tools::font_tool_utils::default_portable_font;

const K_NUM_BLURS: usize = 2;
const K_RADIUS: f32 = 20.0;

// Port of: gm/blurignorexform.cpp#L20-L24 (chrome/m156), BlurIgnoreXformGM::DrawType
#[derive(Clone, Copy, PartialEq, Eq)]
enum DrawType {
    Circle,
    Rect,
    RRect,
}

// Port of: gm/blurignorexform.cpp#L62-L71 (chrome/m156), kBlurFlags
const K_BLUR_FLAGS: [(bool, &str); K_NUM_BLURS] = [(true, "none"), (false, "IgnoreTransform")];

// Port of: gm/blurignorexform.cpp#L72-L76 (chrome/m156), kMatrixScales
const K_MATRIX_SCALES: [(f32, &str); 3] = [
    (1.0, "Identity"),
    (0.5, "Scale = 0.5"),
    (2.0, "Scale = 2.0"),
];

// Port of: gm/blurignorexform.cpp#L15-L70 (chrome/m156), BlurIgnoreXformGM
struct BlurIgnoreXformGm {
    draw_type: DrawType,
    blur_filters: [Option<MaskFilter>; K_NUM_BLURS],
}

impl BlurIgnoreXformGm {
    fn new(draw_type: DrawType) -> Self {
        BlurIgnoreXformGm {
            draw_type,
            blur_filters: [None, None],
        }
    }

    // Port of: gm/blurignorexform.cpp#L55-L68 (chrome/m156), drawOverlay
    fn draw_overlay(canvas: &Canvas) {
        canvas.translate((10.0, 0.0));
        let font = default_portable_font();
        canvas.save();
        for (_, name) in K_BLUR_FLAGS {
            canvas.draw_str(name, (100.0, 0.0), &font, &Paint::default());
            canvas.translate((130.0, 0.0));
        }
        canvas.restore();
        for (_, name) in K_MATRIX_SCALES {
            canvas.draw_str(name, (0.0, 50.0), &font, &Paint::default());
            canvas.translate((0.0, 150.0));
        }
    }
}

impl GM for BlurIgnoreXformGm {
    fn name(&self) -> String {
        let suffix = match self.draw_type {
            DrawType::Circle => "circle",
            DrawType::Rect => "rect",
            DrawType::RRect => "rrect",
        };
        format!("blur_ignore_xform_{suffix}")
    }

    fn size(&mut self) -> ISize {
        ISize::new(375, 475)
    }

    // Port of: gm/blurignorexform.cpp#L30-L36 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        for (i, (respect_ctm, _)) in K_BLUR_FLAGS.iter().enumerate() {
            self.blur_filters[i] = MaskFilter::blur(
                BlurStyle::Normal,
                BlurMask::convert_radius_to_sigma(20.0),
                *respect_ctm,
            );
        }
    }

    // Port of: gm/blurignorexform.cpp#L38-L84 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.set_anti_alias(true);
        canvas.translate((10.0, 25.0));
        canvas.save();
        canvas.translate((80.0, 0.0));
        for i in 0..K_NUM_BLURS {
            let _acr = AutoCanvasRestore::guard(canvas, true);
            canvas.translate((int_to_scalar(i as i32 * 150), 0.0));
            for (scale, _) in K_MATRIX_SCALES {
                canvas.save();
                canvas.scale((scale, scale));
                let coord = 50.0_f32 * 1.0 / scale;
                let rect = Rect::from_xywh(
                    coord - K_RADIUS,
                    coord - K_RADIUS,
                    2.0 * K_RADIUS,
                    2.0 * K_RADIUS,
                );
                let rrect = RRect::new_rect_xy(rect, K_RADIUS / 2.0, K_RADIUS / 2.0);
                paint.set_mask_filter(self.blur_filters[i].clone());
                for j in 0..2 {
                    canvas.save();
                    let offset = int_to_scalar(10 * (1 - j));
                    canvas.translate((offset, offset));
                    match self.draw_type {
                        DrawType::Circle => {
                            canvas.draw_circle((coord, coord), K_RADIUS, &paint);
                        }
                        DrawType::Rect => {
                            canvas.draw_rect(rect, &paint);
                        }
                        DrawType::RRect => {
                            canvas.draw_rrect(rrect, &paint);
                        }
                    }
                    paint.set_mask_filter(None);
                    canvas.restore();
                }
                canvas.restore();
                canvas.translate((0.0, 150.0));
            }
        }
        canvas.restore();
        Self::draw_overlay(canvas);
    }
}

// Port of: gm/blurignorexform.cpp#L86-L88 (chrome/m156), the three DEF_GM registrations
crate::def_gm!(
    BlurIgnoreXformGM_circle = "BlurIgnoreXformGM(BlurIgnoreXformGM::DrawType::kCircle)",
    BlurIgnoreXformGm::new(DrawType::Circle)
);
crate::def_gm!(
    BlurIgnoreXformGM_rect = "BlurIgnoreXformGM(BlurIgnoreXformGM::DrawType::kRect)",
    BlurIgnoreXformGm::new(DrawType::Rect)
);
crate::def_gm!(
    BlurIgnoreXformGM_rrect = "BlurIgnoreXformGM(BlurIgnoreXformGM::DrawType::kRRect)",
    BlurIgnoreXformGm::new(DrawType::RRect)
);
