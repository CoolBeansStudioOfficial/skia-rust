// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/preservefillrule.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;

/**
 * This test originally ensured that the ccpr path cache preserved fill rules properly. CCPR is gone
 * now, but we decided to keep the test.
 */
// Port of: gm/preservefillrule.cpp#L22-L72 (chrome/m156)
struct PreserveFillRuleGM {
    big: bool,
    star_size: i32,
}

impl PreserveFillRuleGM {
    fn new(big: bool) -> Self {
        Self {
            big,
            star_size: if big { 200 } else { 20 },
        }
    }
}

impl GM for PreserveFillRuleGM {
    fn name(&self) -> String {
        let mut name = String::from("preservefillrule");
        name.push_str(if self.big { "_big" } else { "_little" });
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(self.star_size * 2, self.star_size * 2)
    }

    #[allow(clippy::cast_precision_loss)] // int -> SkScalar as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let star_size = self.star_size as f32;
        let star_rect = Rect::from_wh(star_size, star_size);
        let mut star7_winding = crate::tool_utils::make_star(&star_rect, 7, 2);
        star7_winding.set_fill_type(PathFillType::Winding);

        let star7_even_odd = star7_winding
            .make_transform(&Matrix::translate((0.0, star_size)))
            .make_fill_type(PathFillType::EvenOdd);

        let star5_winding = crate::tool_utils::make_star(&star_rect, 5, 2)
            .make_transform(&Matrix::translate((star_size, 0.0)))
            .make_fill_type(PathFillType::Winding);

        let star5_even_odd = star5_winding
            .make_transform(&Matrix::translate((0.0, star_size)))
            .make_fill_type(PathFillType::EvenOdd);

        let mut paint = Paint::default();
        paint.set_color(Color::GREEN);
        paint.set_anti_alias(true);

        canvas.clear(Color::WHITE);
        canvas.draw_path(&star7_winding, &paint);
        canvas.draw_path(&star7_even_odd, &paint);
        canvas.draw_path(&star5_winding, &paint);
        canvas.draw_path(&star5_even_odd, &paint);
    }
}

// Port of: gm/preservefillrule.cpp#L74-L75 (chrome/m156)
crate::def_gm!(
    PreserveFillRuleGM_true = "PreserveFillRuleGM(true)",
    PreserveFillRuleGM::new(true)
);
crate::def_gm!(
    PreserveFillRuleGM_false = "PreserveFillRuleGM(false)",
    PreserveFillRuleGM::new(false)
);
