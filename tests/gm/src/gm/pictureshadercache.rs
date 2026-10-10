// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pictureshadercache.cpp (chrome/m156)

//! A picture shader drawn on a surface in another color space, then on the canvas: the second
//! draw must not reuse the cached tile image of the first.

use crate::GM;
use crate::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::{ColorSpace, named_transfer_fn};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::scalar;
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_raster::surfaces;
use skia_rust_skcms::Matrix3x3;

// Port of: gm/pictureshadercache.cpp#L21-L95 (chrome/m156), class PictureShaderCacheGM
struct PictureShaderCacheGm {
    tile_size: scalar,
    picture: Option<Picture>,
}

impl PictureShaderCacheGm {
    // Port of: gm/pictureshadercache.cpp#L30-L32 (chrome/m156), constructor
    fn new(tile_size: scalar) -> Self {
        Self {
            tile_size,
            picture: None,
        }
    }

    // Port of: gm/pictureshadercache.cpp#L34-L49 (chrome/m156), drawTile
    fn draw_tile(&self, canvas: &Canvas) {
        let t = self.tile_size;
        let mut paint = Paint::default();
        paint.set_color(Color::GREEN);
        paint.set_style(Style::Fill);
        paint.set_anti_alias(true);

        canvas.draw_circle((t / 4.0, t / 4.0), t / 4.0, &paint);
        canvas.draw_rect(Rect::from_xywh(t / 2.0, t / 2.0, t / 2.0, t / 2.0), &paint);

        paint.set_color(Color::RED);
        canvas.draw_line((t / 2.0, t * 1.0 / 3.0), (t / 2.0, t * 2.0 / 3.0), &paint);
        canvas.draw_line((t * 1.0 / 3.0, t / 2.0), (t * 2.0 / 3.0, t / 2.0), &paint);
    }
}

impl GM for PictureShaderCacheGm {
    // Port of: gm/pictureshadercache.cpp#L72-L72 (chrome/m156), getName
    fn name(&self) -> String {
        "pictureshadercache".to_string()
    }

    // Port of: gm/pictureshadercache.cpp#L73-L73 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(100, 100)
    }

    // Port of: gm/pictureshadercache.cpp#L51-L57 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut recorder = PictureRecorder::new();
        let picture_canvas = recorder.begin_recording(
            Rect::from_wh(self.tile_size, self.tile_size),
            false,
        );
        self.draw_tile(picture_canvas);
        self.picture = recorder.finish_recording_as_picture(None);
    }

    // Port of: gm/pictureshadercache.cpp#L75-L95 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(picture) = self.picture.as_ref() else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_shader(picture.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            FilterMode::Nearest,
            None,
            None,
        ));
        {
            // Render in a funny color space that converts green to yellow.
            let green_to_yellow = Matrix3x3 {
                vals: [[1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            };
            let gty = ColorSpace::new_rgb(&named_transfer_fn::SRGB, &green_to_yellow)
                .expect("a parametric colour space");
            let info = ImageInfo::new_n32_premul(ISize::new(100, 100), gty);
            let mut surface =
                surfaces::raster(&info, None::<usize>, None).expect("a raster surface");
            surface
                .canvas()
                .draw_rect(Rect::from_wh(self.tile_size, self.tile_size), &paint);
        }
        // When we draw to the canvas, we should see green because we should *not* reuse the
        // cached picture shader.
        canvas.draw_rect(Rect::from_wh(self.tile_size, self.tile_size), &paint);
    }
}

// Port of: gm/pictureshadercache.cpp#L96-L96 (chrome/m156), DEF_GM(return new PictureShaderCacheGM(100);)
crate::def_gm!(
    #[ignore = "565 mismatch: see notes/gm_pictureshadercache_cpp_PictureShaderCacheGM_100.md"]
    PictureShaderCacheGm_100 = "PictureShaderCacheGM(100)",
    PictureShaderCacheGm::new(100.0)
);
