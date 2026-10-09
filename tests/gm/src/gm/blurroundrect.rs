// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurroundrect.cpp (chrome/m156)
//

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
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::color::Color4f;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::scalar_interp;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};

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

// Port of: gm/blurroundrect.cpp#L36-L55 (chrome/m156), MakeRadial (a 5-pixel shift, unlike blurrect)
fn make_radial_shader() -> Option<skia_rust_core::shader::Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(100.0, 100.0)];
    let tm = TileMode::Clamp;
    let colors = [Color4f::from(Color::RED), Color4f::from(Color::GREEN)];
    let pos = [0.25_f32, 0.75];
    let mut scale = Matrix::new_identity();
    scale.set_scale((0.5, 0.5), None);
    scale.post_translate((5.0, 5.0));
    let center0 = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    let center1 = Point::new(
        scalar_interp(pts[0].x, pts[1].x, 3.0 / 5.0),
        scalar_interp(pts[0].y, pts[1].y, 1.0 / 4.0),
    );
    gradient_shaders::two_point_conical_gradient(
        (center1, (pts[1].x - pts[0].x) / 7.0),
        (center0, (pts[1].x - pts[0].x) / 2.0),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), tm, None),
            Interpolation::default(),
        ),
        Some(&scale),
    )
}

// Port of: gm/blurroundrect.cpp#L57-L95 (chrome/m156), SimpleBlurRoundRectGM
struct SimpleBlurRoundRectGm;

impl GM for SimpleBlurRoundRectGm {
    fn name(&self) -> String {
        "simpleblurroundrect".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1000, 500)
    }

    // Port of: gm/blurroundrect.cpp#L63-L94 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.scale((1.5, 1.5));
        canvas.translate((50.0, 50.0));
        let blur_radii: [f32; 4] = [1.0, 5.0, 10.0, 20.0];
        let corner_radii: [f32; 4] = [1.0, 5.0, 10.0, 20.0];
        let r = Rect::new(0.0, 0.0, 25.0, 25.0);
        for (row, blur_radius) in blur_radii.iter().enumerate() {
            let _acr = AutoCanvasRestore::guard(canvas, true);
            canvas.translate((0.0, (r.height() + 50.0) * row as f32));
            for corner_radius in corner_radii {
                let mut paint = Paint::default();
                paint.set_color(Color::BLACK);
                paint.set_mask_filter(MaskFilter::blur(
                    BlurStyle::Normal,
                    BlurMask::convert_radius_to_sigma(*blur_radius),
                    None,
                ));
                let rrect = RRect::new_rect_xy(r, corner_radius, corner_radius);
                // Even-indexed columns are without a gradient
                canvas.draw_rrect(rrect, &paint);
                canvas.translate((r.width() + 50.0, 0.0));
                // Odd-indexed columns have a gradient
                paint.set_shader(make_radial_shader());
                canvas.draw_rrect(rrect, &paint);
                canvas.translate((r.width() + 50.0, 0.0));
            }
        }
    }
}

// Port of: gm/blurroundrect.cpp#L95 (chrome/m156), DEF_GM(return new SimpleBlurRoundRectGM();)
crate::def_gm!(
    SimpleBlurRoundRectGM_ = "SimpleBlurRoundRectGM()",
    SimpleBlurRoundRectGm
);
