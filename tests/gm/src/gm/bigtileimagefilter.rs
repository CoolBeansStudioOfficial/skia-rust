// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bigtileimagefilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color::Color;
use skia_rust_core::image::Image;
use skia_rust_core::m44::M44;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_effects::image_filters::{image_sampled, tile};
use skia_rust_raster::surfaces;

// Port of: gm/bigtileimagefilter.cpp#L8-L21 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // sizes are small: exact in f32
fn create_circle_texture(size: i32, color: Color) -> Option<Image> {
    let mut surface = surfaces::raster_n32_premul((size, size))?;
    {
        let canvas = surface.canvas();
        canvas.clear(Color::new(0xFF00_0000));
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.set_stroke_width(3.0);
        paint.set_style(Style::Stroke);
        let half = size as f32 / 2.0;
        canvas.draw_circle((half, half), half, &paint);
    }
    surface.image_snapshot()
}

const WIDTH: i32 = 512;
const HEIGHT: i32 = 512;
const BITMAP_SIZE: i32 = 64;
const WIDTH_F: f32 = 512.0;
const HEIGHT_F: f32 = 512.0;
const BITMAP_F: f32 = 64.0;

/// `BigTileImageFilterGM`: a tiled image filter over a region far bigger than its input.
// Port of: gm/bigtileimagefilter.cpp#L23-L66 (chrome/m156)
struct BigTileImageFilterGm {
    red_image: Option<Image>,
    green_image: Option<Image>,
}

impl GM for BigTileImageFilterGm {
    fn name(&self) -> String {
        "bigtileimagefilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/bigtileimagefilter.cpp#L33-L36 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.red_image = create_circle_texture(BITMAP_SIZE, Color::RED);
        self.green_image = create_circle_texture(BITMAP_SIZE, Color::GREEN);
    }

    // Port of: gm/bigtileimagefilter.cpp#L38-L63 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::BLACK);
        {
            let mut p = Paint::default();
            let bound = Rect::from_wh(WIDTH_F, HEIGHT_F);
            let image_source = image_sampled(
                self.red_image.clone(),
                SamplingOptions::from(FilterMode::Linear),
            );
            let tif = tile(
                &Rect::from_wh(BITMAP_F, BITMAP_F),
                &Rect::from_wh(WIDTH_F, HEIGHT_F),
                image_source,
            );
            p.set_image_filter(tif);
            canvas.save_layer(&SaveLayerRec::default().bounds(&bound).paint(&p));
            canvas.restore();
        }
        {
            let mut p2 = Paint::default();
            let bound2 = Rect::from_wh(BITMAP_F, BITMAP_F);
            let tif = tile(
                &Rect::from_wh(BITMAP_F, BITMAP_F),
                &Rect::from_wh(BITMAP_F, BITMAP_F),
                None,
            );
            p2.set_image_filter(tif);
            canvas.translate((320.0, 320.0));
            canvas.save_layer(&SaveLayerRec::default().bounds(&bound2).paint(&p2));
            canvas.set_matrix(&M44::new_identity());
            let bound3 = Rect::from_xywh(320.0, 320.0, BITMAP_F, BITMAP_F);
            if let Some(green) = &self.green_image {
                canvas.draw_image_rect_with_sampling_options(
                    green,
                    Some((&bound2, SrcRectConstraint::Strict)),
                    bound3,
                    SamplingOptions::default(),
                    &Paint::default(),
                );
            }
            canvas.restore();
        }
    }
}

crate::def_gm!(
    BigTileImageFilterGM,
    BigTileImageFilterGm {
        red_image: None,
        green_image: None,
    }
);
