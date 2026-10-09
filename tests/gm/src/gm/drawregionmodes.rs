// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawregionmodes.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::color::Color4f;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::IRect;
use skia_rust_core::region::{Op, Region};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::dash_path_effect;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_effects::image_filters::blur_filter;

// Port of: gm/drawregionmodes.cpp#L9-L43 (chrome/m156)
struct DrawRegionModesGm {
    region: Region,
}

impl DrawRegionModesGm {
    fn new() -> Self {
        Self {
            region: Region::new(),
        }
    }
}

impl GM for DrawRegionModesGm {
    fn name(&self) -> String {
        "drawregionmodes".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(375, 500)
    }

    // Port of: gm/drawregionmodes.cpp#L17-L21 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.region
            .op_rect(IRect::from_ltrb(50, 50, 100, 100), Op::Union);
        self.region
            .op_rect(IRect::from_ltrb(50, 100, 150, 150), Op::Union);
    }

    // Port of: gm/drawregionmodes.cpp#L22-L58 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::GREEN);
        let mut paint = Paint::default();
        paint.set_style(Style::Fill);
        paint.set_color(Color::RED);
        paint.set_anti_alias(true);
        canvas.save();
        canvas.translate((-50.0, 75.0));
        canvas.rotate(-45.0, None);
        canvas.draw_region(&self.region, &paint);
        canvas.translate((125.0, 125.0));
        paint.set_image_filter(blur_filter::blur(5.0, 5.0, TileMode::Decal, None, None));
        canvas.draw_region(&self.region, &paint);
        canvas.translate((-125.0, 125.0));
        paint.set_image_filter(None);
        paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 5.0, None));
        canvas.draw_region(&self.region, &paint);
        canvas.translate((-125.0, -125.0));
        paint.set_mask_filter(None);
        paint.set_style(Style::Stroke);
        let intervals = [5.0_f32, 5.0];
        paint.set_path_effect(dash_path_effect::new(&intervals, 2.5));
        canvas.draw_region(&self.region, &paint);
        canvas.restore();
        canvas.translate((100.0, 325.0));
        paint.set_path_effect(None);
        paint.set_style(Style::Fill);
        let points = [Point::new(50.0, 50.0), Point::new(150.0, 150.0)];
        let colors = [
            Color4f::new(0.0, 0.0, 1.0, 1.0),
            Color4f::new(1.0, 1.0, 0.0, 1.0),
        ];
        paint.set_shader(shaders::linear_gradient(
            (points[0], points[1]),
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        canvas.draw_region(&self.region, &paint);
    }
}

crate::def_gm!(DrawRegionModesGM, DrawRegionModesGm::new());
