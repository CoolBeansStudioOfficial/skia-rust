// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/surface.cpp (chrome/m156)
//
// Not ported: `SurfacePropsGM` (text and gradients). The `DEF_SURFACE_TESTS` GMs are not in the
// inventory.

use crate::prelude::*;
use crate::tool_utils;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_raster::surfaces;

// Port of: gm/surface.cpp#L178-L211 (chrome/m156)
struct NewSurfaceGm;

impl NewSurfaceGm {
    fn draw_into(canvas: &Canvas) {
        canvas.draw_color(Color::RED, None);
    }
}

impl GM for NewSurfaceGm {
    fn name(&self) -> String {
        "surfacenew".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(300, 140)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let info = ImageInfo::new_n32_premul((100, 100), None);

        let mut surf = tool_utils::make_surface(canvas, &info, None).expect("a surface");
        Self::draw_into(surf.canvas());

        let image = surf.image_snapshot().expect("a snapshot");
        canvas.draw_image(&image, (10.0, 10.0), None);

        let mut surf2 = surf.new_surface(&info).expect("a surface");
        Self::draw_into(surf2.canvas());

        // Assert that the props were communicated transitively through the first image
        debug_assert_eq!(surf.props(), surf2.props());

        let image2 = surf2.image_snapshot().expect("a snapshot");
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        canvas.draw_image(&image2, (10.0 + image.width() as f32 + 10.0, 10.0), None);
    }
}

// Port of: gm/surface.cpp#L213 (chrome/m156)
crate::def_gm!(NewSurfaceGM, NewSurfaceGm);

// Port of: gm/surface.cpp#L379-L418 (chrome/m156)
crate::def_simple_gm!(snap_with_mips, canvas, 80, 75, {
    const PAD: i32 = 8;

    let canvas_info = canvas.image_info();
    let ct = if canvas_info.color_type() == ColorType::Unknown {
        ColorType::RGBA8888
    } else {
        canvas_info.color_type()
    };
    let ii = ImageInfo::new((32, 32), ct, AlphaType::Premul, canvas_info.color_space());
    let mut surface = surfaces::raster(&ii, None, None).expect("a surface");

    #[allow(clippy::cast_precision_loss)] // surface->width() *2/5.f
    let mut next_image = |color: Color| {
        let (w, h) = (surface.width() as f32, surface.height() as f32);
        surface.canvas().clear(color);
        let mut paint = Paint::default();
        paint.set_color(Color::new(!u32::from(color) | 0xFF00_0000));
        surface.canvas().draw_rect(
            Rect::new(w * 2.0 / 5.0, h * 2.0 / 5.0, w * 3.0 / 5.0, h * 3.0 / 5.0),
            &paint,
        );
        surface
            .image_snapshot()
            .expect("a snapshot")
            .with_default_mipmaps()
            .expect("an image")
    };

    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);

    #[allow(clippy::cast_precision_loss)] // ii.width() + kPad
    let step = (ii.width() + PAD) as f32;
    canvas.save();
    for _ in 0..3 {
        canvas.save();
        let colors = [Color::new(0xFFF0_F0F0), Color::BLUE];
        for color in colors {
            let image = next_image(color);
            canvas.draw_image_with_sampling_options(&image, (0.0, 0.0), sampling, None);
            canvas.translate((step, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, step));
        canvas.scale((0.4, 0.4));
    }
    canvas.restore();
});
