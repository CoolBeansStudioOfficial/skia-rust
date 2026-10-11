// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagesource2.cpp (chrome/m156)

// This GM reproduces the issue in crbug.com/472795. The SkImageSource image
// is shifted for high quality mode between cpu and gpu.

// int casts and index loops mirror the C++
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::image::Image;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_effects::image_filters::image;
use skia_rust_raster::surfaces;

// Port of: gm/imagesource2.cpp#L27-L35 (chrome/m156)
const IMAGE_SIZE: i32 = 503;

// Port of: gm/imagesource2.cpp#L19-L25 (chrome/m156)
#[derive(Debug)]
pub struct ImageSourceGm {
    suffix: &'static str,
    sampling: SamplingOptions,
    image: Option<Image>,
}

impl ImageSourceGm {
    // Port of: gm/imagesource2.cpp#L19-L25 (chrome/m156)
    #[must_use]
    pub fn new(suffix: &'static str, sampling: SamplingOptions) -> Self {
        Self {
            suffix,
            sampling,
            image: None,
        }
    }
}

impl GM for ImageSourceGm {
    // Port of: gm/imagesource2.cpp#L27-L35 (chrome/m156)
    fn name(&self) -> String {
        format!("imagesrc2_{}", self.suffix)
    }

    fn size(&mut self) -> ISize {
        ISize::new(256, 256)
    }

    fn bg_color(&self) -> Color {
        Color::WHITE
    }

    // Create an image with high frequency vertical stripes
    // Port of: gm/imagesource2.cpp#L43-L63 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        let g_colors = [
            Color::RED,
            Color::GRAY,
            Color::GREEN,
            Color::GRAY,
            Color::BLUE,
            Color::GRAY,
            Color::CYAN,
            Color::GRAY,
            Color::MAGENTA,
            Color::GRAY,
            Color::YELLOW,
            Color::GRAY,
            Color::WHITE,
            Color::GRAY,
        ];

        let mut surface =
            surfaces::raster_n32_premul((IMAGE_SIZE, IMAGE_SIZE)).expect("a raster surface");
        let canvas = surface.canvas();

        let mut cur_color = 0;

        let mut x = 0;
        while x < IMAGE_SIZE {
            let r = Rect::from_xywh(x as f32, 0.0, 3.0, IMAGE_SIZE as f32);
            let mut p = Paint::default();
            p.set_color(g_colors[cur_color]);
            canvas.draw_rect(r, &p);

            cur_color = (cur_color + 1) % g_colors.len();
            x += 3;
        }

        self.image = surface.image_snapshot();
    }

    // Port of: gm/imagesource2.cpp#L65-L78 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let src_rect = Rect::from_ltrb(0.0, 0.0, IMAGE_SIZE as f32, IMAGE_SIZE as f32);
        let dst_rect = Rect::from_ltrb(0.75, 0.75, 225.75, 225.75);

        let mut p = Paint::default();
        p.set_image_filter(image(self.image.clone(), src_rect, dst_rect, self.sampling));

        canvas.save_layer(&SaveLayerRec::default().paint(&p));
        canvas.restore();
    }
}

// Port of: gm/imagesource2.cpp#L94-L94 (chrome/m156)
crate::def_gm!(
    ImageSourceGM_none = "ImageSourceGM(\"none\", SkSamplingOptions())",
    ImageSourceGm::new("none", SamplingOptions::default())
);

// Port of: gm/imagesource2.cpp#L97-L97 (chrome/m156)
crate::def_gm!(
    ImageSourceGM_high = "ImageSourceGM(\"high\", SkSamplingOptions({1/3.0f, 1/3.0f}))",
    ImageSourceGm::new(
        "high",
        SamplingOptions::from(CubicResampler {
            b: 1.0 / 3.0,
            c: 1.0 / 3.0
        })
    )
);

// Port of: gm/imagesource2.cpp#L95-L95 (chrome/m156)
crate::def_gm!(
    ImageSourceGM_low = "ImageSourceGM(\"low\", SkSamplingOptions(SkFilterMode::kLinear))",
    ImageSourceGm::new(
        "low",
        SamplingOptions::from(skia_rust_core::sampling_options::FilterMode::Linear)
    )
);

// Port of: gm/imagesource2.cpp#L98-L100 (chrome/m156)
crate::def_gm!(
    ImageSourceGM_med = "ImageSourceGM(\"med\", SkSamplingOptions(SkFilterMode::kLinear, SkMipmapMode::kLinear))",
    ImageSourceGm::new(
        "med",
        SamplingOptions::new(
            skia_rust_core::sampling_options::FilterMode::Linear,
            skia_rust_core::sampling_options::MipmapMode::Linear
        )
    )
);
