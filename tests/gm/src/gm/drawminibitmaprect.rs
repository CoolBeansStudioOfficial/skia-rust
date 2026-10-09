// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawminibitmaprect.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements
)]

use crate::prelude::*;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color::Color4f;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::math_priv::next_log2;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::surfaces;

// Port of: gm/drawminibitmaprect.cpp#L9-L43 (chrome/m156)
fn makebm(w: i32, h: i32) -> Option<Image> {
    let mut surface = surfaces::raster(&ImageInfo::new_n32_premul((w, h), None), None, None)?;
    let w_scalar = w as f32;
    let h_scalar = h as f32;
    let pt = Point::new(w_scalar / 2.0, h_scalar / 2.0);
    let radius = 4.0 * w_scalar.max(h_scalar);
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(1.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(1.0, 0.0, 1.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(0.0, 1.0, 1.0, 1.0),
        Color4f::new(1.0, 0.0, 0.0, 1.0),
    ];
    let pos: [f32; 7] = [
        0.0,
        1.0 / 6.0,
        2.0 * 1.0 / 6.0,
        3.0 * 1.0 / 6.0,
        4.0 * 1.0 / 6.0,
        5.0 * 1.0 / 6.0,
        1.0,
    ];
    let mut paint = Paint::default();
    let mut rect = Rect::from_wh(w_scalar, h_scalar);
    let mut mat = Matrix::new_identity();
    {
        let canvas = surface.canvas();
        for _ in 0..4 {
            paint.set_shader(shaders::radial_gradient(
                (pt, radius),
                &Gradient::new(
                    Colors::new(&colors, Some(&pos), TileMode::Repeat, None),
                    Interpolation::default(),
                ),
                Some(&mat),
            ));
            canvas.draw_rect(rect, &paint);
            rect.inset((w_scalar / 8.0, h_scalar / 8.0));
            mat.post_scale((1.0 / 4.0, 1.0 / 4.0), None);
        }
    }
    surface.image_snapshot()
}

// Port of: gm/drawminibitmaprect.cpp#L45-L50 (chrome/m156)
const G_SIZE: i32 = 1024;
// Port of: gm/drawminibitmaprect.cpp#L46 (chrome/m156)
const G_SURFACE_SIZE: i32 = 2048;

// Port of: gm/drawminibitmaprect.cpp#L51-L115 (chrome/m156)
struct DrawMiniBitmapRectGm {
    anti_alias: bool,
    name: String,
    image: Option<Image>,
}

impl DrawMiniBitmapRectGm {
    fn new(anti_alias: bool) -> Self {
        let mut name = String::from("drawminibitmaprect");
        if anti_alias {
            name.push_str("_aa");
        }
        Self {
            anti_alias,
            name,
            image: None,
        }
    }
}

impl GM for DrawMiniBitmapRectGm {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn size(&mut self) -> ISize {
        ISize::new(G_SIZE, G_SIZE)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        if self.image.is_none() {
            // `ToolUtils::MakeTextureImage` returns the image itself on a raster canvas.
            self.image = makebm(G_SURFACE_SIZE, G_SURFACE_SIZE);
        }
        let image = self.image.as_ref().expect("the image is made above");
        let dst_rect = Rect::from_ltrb(0.0, 0.0, 64.0, 64.0);
        let k_max_src_rect_size: i32 = 1 << (next_log2(G_SURFACE_SIZE as u32) + 2);
        const K_PAD_X: i32 = 30;
        const K_PAD_Y: i32 = 40;
        let mut row_count = 0;
        canvas.translate((K_PAD_X as f32, K_PAD_Y as f32));
        canvas.save();
        let mut random = Random::default();
        let mut paint = Paint::default();
        paint.set_anti_alias(self.anti_alias);
        let mut w = 1;
        while w <= k_max_src_rect_size {
            let mut h = 1;
            while h <= k_max_src_rect_size {
                let src_rect =
                    IRect::from_xywh((G_SURFACE_SIZE - w) / 2, (G_SURFACE_SIZE - h) / 2, w, h);
                canvas.save();
                match random.next_u() % 3 {
                    0 => {
                        canvas.rotate(random.next_f() * 10.0, None);
                    }
                    1 => {
                        canvas.rotate(-random.next_f() * 10.0, None);
                    }
                    _ => {}
                }
                canvas.draw_image_rect(
                    image,
                    Some((&Rect::from_irect(src_rect), SrcRectConstraint::Fast)),
                    dst_rect,
                    &paint,
                );
                canvas.restore();
                canvas.translate((dst_rect.width() + 1.0 * K_PAD_X as f32, 0.0));
                row_count += 1;
                if (dst_rect.width() + 2.0 * K_PAD_X as f32) * row_count as f32 > G_SIZE as f32 {
                    canvas.restore();
                    canvas.translate((0.0, dst_rect.height() + 1.0 * K_PAD_Y as f32));
                    canvas.save();
                    row_count = 0;
                }
                h *= 3;
            }
            w *= 3;
        }
        canvas.restore();
    }
}

// Port of: gm/drawminibitmaprect.cpp#L117 (chrome/m156)
crate::def_gm!(
    DrawMiniBitmapRectGM_true = "DrawMiniBitmapRectGM(true)",
    DrawMiniBitmapRectGm::new(true)
);
// Port of: gm/drawminibitmaprect.cpp#L118 (chrome/m156)
crate::def_gm!(
    DrawMiniBitmapRectGM_false = "DrawMiniBitmapRectGM(false)",
    DrawMiniBitmapRectGm::new(false)
);
