// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/spritebitmap.cpp (chrome/m156)

// Compare output of drawSprite and drawBitmap (esp. clipping and imagefilters)

// The int-to-scalar casts of small pixel sizes and offsets mirror the C++ arithmetic.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/spritebitmap.cpp#L8-L18 (chrome/m156), make_bm
fn make_bm() -> Bitmap {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((100, 100), None);
    bm.erase_color(Color::BLUE);

    let canvas = Canvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::RED);
    canvas.draw_circle((50.0, 50.0), 50.0, &paint);
    drop(canvas);
    bm
}

// Port of: gm/spritebitmap.cpp#L20-L40 (chrome/m156), draw_1_bitmap
fn draw_1_bitmap(
    canvas: &Canvas,
    bm: &Bitmap,
    do_clip: bool,
    dx: i32,
    dy: i32,
    filter: Option<ImageFilter>,
) {
    let _acr = AutoCanvasRestore::guard(canvas, true);
    let mut paint = Paint::default();

    let mut clip_r = Rect::from_xywh(dx as f32, dy as f32, bm.width() as f32, bm.height() as f32);

    paint.set_image_filter(filter);
    clip_r.inset((5.0, 5.0));

    canvas.translate(((bm.width() + 20) as f32, 0.0));

    if do_clip {
        canvas.save();
        canvas.clip_rect(clip_r, None, None);
    }
    canvas.draw_image_with_sampling_options(
        bm.as_image().expect("the bitmap image"),
        (dx as f32, dy as f32),
        SamplingOptions::default(),
        Some(&paint),
    );
    if do_clip {
        canvas.restore();
    }
}

// Port of: gm/spritebitmap.cpp#L42-L70 (chrome/m156), class SpriteBitmapGM
#[derive(Debug, Default)]
pub struct SpriteBitmapGm;

impl GM for SpriteBitmapGm {
    fn name(&self) -> String {
        "spritebitmap".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/spritebitmap.cpp#L49-L63 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let bm = make_bm();

        let mut dy: i32 = 10;
        let dx: i32 = 10;

        let sigma = 8.0;
        let filter = blur(sigma, sigma, TileMode::Decal, None, None);

        draw_1_bitmap(canvas, &bm, false, dx, dy, None);
        dy += bm.height() + 20;
        draw_1_bitmap(canvas, &bm, false, dx, dy, filter.clone());
        dy += bm.height() + 20;
        draw_1_bitmap(canvas, &bm, true, dx, dy, None);
        dy += bm.height() + 20;
        draw_1_bitmap(canvas, &bm, true, dx, dy, filter);
    }
}

// Port of: gm/spritebitmap.cpp#L71-L71 (chrome/m156), DEF_GM( return new SpriteBitmapGM; )

crate::def_gm!(
    #[ignore = "see notes/gm_spritebitmap_cpp_SpriteBitmapGM.md"]
    SpriteBitmapGM,
    SpriteBitmapGm
);
