// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/showmiplevels.cpp (chrome/m156)

//! Shows which hand-made mip levels a downscaled image samples: each level is a solid colour.

use crate::GM;
use crate::canvas::Canvas;
use crate::tool_utils::get_resource_as_image;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::mipmap_builder::MipmapBuilder;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::surfaces::wrap_pixels;

// Port of: gm/showmiplevels.cpp#L38-L101 (chrome/m156), class ShowMipLevels
struct ShowMipLevelsGm {
    img: Option<Image>,
}

impl ShowMipLevelsGm {
    // Port of: gm/showmiplevels.cpp#L86-L100 (chrome/m156), draw_downscaling
    fn draw_downscaling(canvas: &Canvas, img: &Image, sampling: SamplingOptions) -> f32 {
        let _acr = AutoCanvasRestore::guard(canvas, true);

        let mut paint = Paint::default();
        let r = Rect::from_ltrb(0.0, 0.0, 150.0, 150.0);
        let mut scale = 1.0_f32;
        while scale >= 0.1_f32 {
            let matrix = Matrix::scale((scale, scale));
            paint.set_shader(img.to_shader(
                (TileMode::Repeat, TileMode::Repeat),
                sampling,
                &matrix,
            ));
            canvas.draw_rect(r, &paint);
            canvas.translate((r.width() + 10.0, 0.0));
            scale *= 0.7_f32;
        }
        r.height() + 10.0
    }
}

impl GM for ShowMipLevelsGm {
    // Port of: gm/showmiplevels.cpp#L43-L43 (chrome/m156), getName
    fn name(&self) -> String {
        "showmiplevels_explicit".to_string()
    }

    // Port of: gm/showmiplevels.cpp#L45-L45 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1130, 970)
    }

    // Port of: gm/showmiplevels.cpp#L47-L63 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let img = get_resource_as_image("images/ship.png").expect("images/ship.png");
        // makeWithMips only works on raster for now
        let img = img.to_raster_image().expect("a raster image of ship.png");

        let colors = [Color::RED, Color::GREEN, Color::BLUE];

        let mut builder = MipmapBuilder::new(img.image_info());
        // The number of levels is derived from the size of the image, so we intentionally
        // pick an image that's big enough to support 3+ levels.
        assert!(builder.count_levels() >= i32::try_from(colors.len()).expect("3 colours"));
        for i in 0..builder.count_levels() {
            let (info, row_bytes, pixels) = builder.level_mut(i).expect("a level of the builder");
            let mut surf = wrap_pixels(&info, pixels, row_bytes, None)
                .expect("a surface over the level's pixels");
            let color = colors[usize::try_from(i).expect("level index") % colors.len()];
            surf.canvas().draw_color(Color4f::from(color), None);
        }
        self.img = Some(builder.attach_to(&img));
    }

    // Port of: gm/showmiplevels.cpp#L65-L78 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(img) = self.img.clone() else {
            return;
        };
        canvas.draw_color(Color4f::from(Color::new(0xFFDD_DDDD)), None);

        canvas.translate((10.0, 10.0));
        for mm in [MipmapMode::None, MipmapMode::Nearest, MipmapMode::Linear] {
            for fm in [FilterMode::Nearest, FilterMode::Linear] {
                let dy = Self::draw_downscaling(canvas, &img, SamplingOptions::new(fm, mm));
                canvas.translate((0.0, dy));
            }
        }
    }
}

// Port of: gm/showmiplevels.cpp#L102-L102 (chrome/m156), DEF_GM(return new ShowMipLevels;)
crate::def_gm!(ShowMipLevels, ShowMipLevelsGm { img: None });
