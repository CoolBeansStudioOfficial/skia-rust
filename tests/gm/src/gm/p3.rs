// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/p3.cpp (chrome/m156)
//
// Only `p3_ovals` is ported here. Its `compare_pixel` checks only print a message on a mismatch
// (`SkDebugf`); they never draw, so they are not part of the output and are not ported.

use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_effects::dash_path_effect;

// Test cases that exercise each Op in GrOvalOpFactory.cpp
// Port of: gm/p3.cpp#L341-L409 (chrome/m156), DEF_SIMPLE_GM(p3_ovals)
crate::def_simple_gm!(
    #[ignore = "see notes/gm_p3_cpp_p3_ovals.md"]
    p3_ovals,
    canvas,
    450,
    320,
    {
        let p3 = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3)
            .expect("the Display P3 color space");

        // Draw a circle (CircleOp)
        {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color4f(Color4f::new(1.0, 0.0, 0.0, 1.0), &p3);

            canvas.draw_circle((40.0, 40.0), 30.0, &paint);
        }

        canvas.translate((0.0, 80.0));

        // Draw an oval (EllipseOp)
        {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color4f(Color4f::new(1.0, 0.0, 0.0, 1.0), &p3);

            canvas.draw_oval(Rect::from_ltrb(20.0, 10.0, 60.0, 70.0), &paint);
        }

        canvas.translate((0.0, 80.0));

        // Draw a butt-capped dashed circle (ButtCappedDashedCircleOp)
        {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color4f(Color4f::new(1.0, 0.0, 0.0, 1.0), &p3);
            paint.set_style(Style::Stroke);
            let intervals = [70.0_f32, 10.0];
            paint.set_path_effect(dash_path_effect::new(&intervals, 0.0));
            paint.set_stroke_width(10.0);

            canvas.draw_circle((40.0, 40.0), 30.0, &paint);
        }

        canvas.translate((0.0, 80.0));

        // Draw an oval with rotation (DIEllipseOp)
        {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color4f(Color4f::new(1.0, 0.0, 0.0, 1.0), &p3);

            canvas.save();
            canvas.translate((40.0, 40.0));
            canvas.rotate(45.0, None);
            canvas.draw_oval(Rect::from_ltrb(-20.0, -30.0, 20.0, 30.0), &paint);
            canvas.restore();
        }

        canvas.translate((0.0, 80.0));
    }
);
