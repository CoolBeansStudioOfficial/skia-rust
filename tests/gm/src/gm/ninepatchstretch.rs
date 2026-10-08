// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/ninepatchstretch.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

/// `ToolUtils::makeSurface`: a surface like the canvas's, or a raster one.
// Port of: tools/ToolUtils.cpp#L504-L512 (chrome/m156)
fn make_surface_for(root: &Canvas, info: &ImageInfo) -> Surface<'static> {
    root.new_surface(info, None)
        .or_else(|| surfaces::raster(info, None, None))
        .expect("a surface")
}

// Port of: gm/ninepatchstretch.cpp#L27-L30 (chrome/m156)
fn make_surface(root: &Canvas, n: i32) -> Surface<'static> {
    let info = ImageInfo::new_n32_premul((n, n), None);
    make_surface_for(root, &info)
}

// Port of: gm/ninepatchstretch.cpp#L32-L60 (chrome/m156)
fn make_image(root: &Canvas, center: &mut IRect) -> Option<Image> {
    const K_FIXED: i32 = 28;
    const K_STRETCHY: i32 = 8;
    const K_SIZE: i32 = 2 * K_FIXED + K_STRETCHY;

    let mut surface = make_surface(root, K_SIZE);
    {
        let canvas = surface.canvas();

        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        let (size, fixed, stretchy) = (K_SIZE as scalar, K_FIXED as scalar, K_STRETCHY as scalar);
        let mut r = Rect::from_wh(size, size);
        let stroke_width: scalar = 6.0;
        let radius = fixed - stroke_width / 2.0;

        *center = IRect::from_xywh(K_FIXED, K_FIXED, K_STRETCHY, K_STRETCHY);

        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        paint.set_color(Color::new(0xFFFF_0000));
        canvas.draw_round_rect(r, radius, radius, &paint);
        r = Rect::from_xywh(fixed, 0.0, stretchy, size);
        paint.set_color(Color::new(0x8800_FF00));
        canvas.draw_rect(r, &paint);
        r = Rect::from_xywh(0.0, fixed, size, stretchy);
        paint.set_color(Color::new(0x8800_00FF));
        canvas.draw_rect(r, &paint);
    }

    surface.image_snapshot()
}

// Port of: gm/ninepatchstretch.cpp#L62-L109 (chrome/m156)
struct NinePatchStretchGm {
    image: Option<Image>,
    center: IRect,
}

impl NinePatchStretchGm {
    fn new() -> NinePatchStretchGm {
        NinePatchStretchGm {
            image: None,
            center: IRect::new_empty(),
        }
    }
}

impl GM for NinePatchStretchGm {
    fn name(&self) -> String {
        "ninepatch-stretch".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(760, 800)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        if self.image.as_ref().is_none_or(|i| !i.is_valid()) {
            self.image = make_image(canvas, &mut self.center);
        }
        let image = self.image.as_ref().expect("image");

        // amount of bm that should not be stretched (unless we have to)
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        let fixed = (image.width() - self.center.width()) as scalar;

        let size: [(scalar, scalar); 4] = [
            (fixed * 4.0 / 5.0, fixed * 4.0 / 5.0), // shrink in both axes
            (fixed * 4.0 / 5.0, fixed * 4.0),       // shrink in X
            (fixed * 4.0, fixed * 4.0 / 5.0),       // shrink in Y
            (fixed * 4.0, fixed * 4.0),
        ];

        canvas.draw_image(image, (10.0, 10.0), None);

        let x: scalar = 100.0;
        let y: scalar = 100.0;

        for fm in [FilterMode::Linear, FilterMode::Nearest] {
            for iy in 0..2usize {
                for ix in 0..2usize {
                    let i = ix * 2 + iy;
                    #[allow(clippy::cast_precision_loss)] // int * SkScalar
                    let r = Rect::from_xywh(
                        x + ix as scalar * fixed,
                        y + iy as scalar * fixed,
                        size[i].0,
                        size[i].1,
                    );
                    canvas.draw_image_nine(image, self.center, r, fm, None);
                }
            }
            canvas.translate((0.0, 400.0));
        }
    }
}

// Port of: gm/ninepatchstretch.cpp#L111 (chrome/m156)
crate::def_gm!(NinePatchStretchGM, NinePatchStretchGm::new());
