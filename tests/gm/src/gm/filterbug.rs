// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/filterbug.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/filterbug.cpp#L25-L42 (chrome/m156)
fn make_image(first_black_row: i32, last_black_row: i32) -> Option<Image> {
    const WIDTH: i32 = 25;
    const HEIGHT: i32 = 27;

    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((WIDTH, HEIGHT), None);
    bm.erase_color(Color::WHITE);
    for y in first_black_row..last_black_row {
        for x in 0..WIDTH {
            bm.set_addr32(x, y, pack_argb32(0xFF, 0x0, 0x0, 0x0));
        }
    }

    let _ = bm.set_alpha_type(AlphaType::Opaque);
    bm.set_immutable();

    bm.as_image()
}

// GM to reproduce crbug.com/673261.
// Port of: gm/filterbug.cpp#L45-L109 (chrome/m156)
struct FilterBugGm {
    top: Option<Image>,
    bot: Option<Image>,
}

impl GM for FilterBugGm {
    fn bg_color(&self) -> Color {
        Color::RED
    }

    fn name(&self) -> String {
        "filterbug".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(150, 150)
    }

    fn on_once_before_draw(&mut self) {
        // The top texture has 5 black rows on top and then 22 white rows on the bottom
        self.top = make_image(0, 5);
        // The bottom texture has 5 black rows on the bottom and then 22 white rows on the top
        self.bot = make_image(22, 27);
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        const DO_AA: bool = true;
        let sampling = SamplingOptions::from(CubicResampler::mitchell());

        {
            let r1 = Rect::from_xywh(50.0, 0.0, 50.0, 50.0);
            let mut p1 = Paint::default();
            p1.set_anti_alias(DO_AA);
            let mut local_mat = Matrix::new_identity();
            local_mat.set_scale_translate((2.0, 2.0), (50.0, 0.0));
            p1.set_shader(self.top.as_ref().expect("an image").to_shader(
                (TileMode::Repeat, TileMode::Repeat),
                sampling,
                &local_mat,
            ));

            canvas.draw_rect(r1, &p1);
        }

        {
            let r2 = Rect::from_xywh(50.0, 50.0, 50.0, 36.0);

            let mut p2 = Paint::default();
            p2.set_color(Color::WHITE);
            p2.set_anti_alias(DO_AA);

            canvas.draw_rect(r2, &p2);
        }

        {
            let r3 = Rect::from_xywh(50.0, 86.0, 50.0, 50.0);

            let mut p3 = Paint::default();
            p3.set_anti_alias(DO_AA);
            let mut local_mat = Matrix::new_identity();
            local_mat.set_scale_translate((2.0, 2.0), (50.0, 86.0));
            p3.set_shader(self.bot.as_ref().expect("an image").to_shader(
                (TileMode::Repeat, TileMode::Repeat),
                sampling,
                &local_mat,
            ));

            canvas.draw_rect(r3, &p3);
        }
    }
}

// Port of: gm/filterbug.cpp#L113 (chrome/m156)
crate::def_gm!(
    FilterBugGM,
    FilterBugGm {
        top: None,
        bot: None
    }
);
