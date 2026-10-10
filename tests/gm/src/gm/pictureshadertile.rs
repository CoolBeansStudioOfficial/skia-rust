// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pictureshadertile.cpp (chrome/m156)

//! Picture shaders whose tile rect, local matrix and picture offset differ, tiled in repeat mode.

use crate::GM;
use crate::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::picture_shader::PictureShaderExt;

// Port of: gm/pictureshadertile.cpp#L17-L19 (chrome/m156)
const K_PICTURE_SIZE: scalar = 1.0;
// Port of: gm/pictureshadertile.cpp#L18 (chrome/m156)
const K_FILL_SIZE: scalar = 100.0;
// Port of: gm/pictureshadertile.cpp#L19 (chrome/m156)
const K_ROW_SIZE: usize = 6;

/// `tiles`: the tile rect (x, y, w, h, in picture units) and the local matrix offset of each
/// shader.
// Port of: gm/pictureshadertile.cpp#L21-L50 (chrome/m156)
const TILES: [(scalar, scalar, scalar, scalar, scalar, scalar); 27] = [
    (0.0, 0.0, 1.0, 1.0, 0.0, 0.0),
    (-0.5, -0.5, 1.0, 1.0, 0.0, 0.0),
    (0.5, 0.5, 1.0, 1.0, 0.0, 0.0),
    (0.0, 0.0, 1.5, 1.5, 0.0, 0.0),
    (-0.5, -0.5, 1.5, 1.5, 0.0, 0.0),
    (0.5, 0.5, 1.5, 1.5, 0.0, 0.0),
    (0.0, 0.0, 0.5, 0.5, 0.0, 0.0),
    (0.25, 0.25, 0.5, 0.5, 0.0, 0.0),
    (-0.25, -0.25, 0.5, 0.5, 0.0, 0.0),
    (0.0, 0.0, 1.0, 1.0, 0.5, 0.5),
    (-0.5, -0.5, 1.0, 1.0, 0.5, 0.5),
    (0.5, 0.5, 1.0, 1.0, 0.5, 0.5),
    (0.0, 0.0, 1.5, 1.5, 0.5, 0.5),
    (-0.5, -0.5, 1.5, 1.5, 0.5, 0.5),
    (0.5, 0.5, 1.5, 1.5, 0.5, 0.5),
    (0.0, 0.0, 1.5, 1.0, 0.0, 0.0),
    (-0.5, -0.5, 1.5, 1.0, 0.0, 0.0),
    (0.5, 0.5, 1.5, 1.0, 0.0, 0.0),
    (0.0, 0.0, 0.5, 1.0, 0.0, 0.0),
    (0.25, 0.25, 0.5, 1.0, 0.0, 0.0),
    (-0.25, -0.25, 0.5, 1.0, 0.0, 0.0),
    (0.0, 0.0, 1.0, 1.5, 0.0, 0.0),
    (-0.5, -0.5, 1.0, 1.5, 0.0, 0.0),
    (0.5, 0.5, 1.0, 1.5, 0.0, 0.0),
    (0.0, 0.0, 1.0, 0.5, 0.0, 0.0),
    (0.25, 0.25, 1.0, 0.5, 0.0, 0.0),
    (-0.25, -0.25, 1.0, 0.5, 0.0, 0.0),
];

// Port of: gm/pictureshadertile.cpp#L52-L80 (chrome/m156), draw_scene
fn draw_scene(canvas: &Canvas, picture_size: scalar) {
    canvas.clear(Color::WHITE);

    let mut paint = Paint::default();
    paint.set_style(Style::Fill);
    paint.set_anti_alias(true);
    paint.set_color(Color::GREEN);
    canvas.draw_circle(
        (picture_size / 4.0, picture_size / 4.0),
        picture_size / 4.0,
        &paint,
    );
    paint.set_color(Color::BLUE);
    canvas.draw_rect(
        Rect::from_xywh(
            picture_size / 2.0,
            picture_size / 2.0,
            picture_size / 2.0,
            picture_size / 2.0,
        ),
        &paint,
    );
    paint.set_color(Color::RED);
    canvas.draw_line(
        (picture_size / 2.0, picture_size * 1.0 / 3.0),
        (picture_size / 2.0, picture_size * 2.0 / 3.0),
        &paint,
    );
    canvas.draw_line(
        (picture_size * 1.0 / 3.0, picture_size / 2.0),
        (picture_size * 2.0 / 3.0, picture_size / 2.0),
        &paint,
    );
    paint.set_color(Color::BLACK);
    paint.set_style(Style::Stroke);
    canvas.draw_rect(Rect::from_wh(picture_size, picture_size), &paint);
}

// Port of: gm/pictureshadertile.cpp#L82-L160 (chrome/m156), class PictureShaderTileGM
#[derive(Default)]
struct PictureShaderTileGm {
    shaders: Vec<Option<Shader>>,
}

impl GM for PictureShaderTileGm {
    // Port of: gm/pictureshadertile.cpp#L86-L86 (chrome/m156), getName
    fn name(&self) -> String {
        "pictureshadertile".to_string()
    }

    // Port of: gm/pictureshadertile.cpp#L87-L87 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(800, 600)
    }

    // Port of: gm/pictureshadertile.cpp#L89-L124 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut recorder = PictureRecorder::new();
        let picture_canvas =
            recorder.begin_recording(Rect::from_wh(K_PICTURE_SIZE, K_PICTURE_SIZE), false);
        draw_scene(picture_canvas, K_PICTURE_SIZE);
        let picture = recorder.finish_recording_as_picture(None);

        let offset = Point::new(100.0, 100.0);
        let picture_canvas = recorder.begin_recording(
            Rect::from_xywh(offset.x, offset.y, K_PICTURE_SIZE, K_PICTURE_SIZE),
            false,
        );
        picture_canvas.translate((offset.x, offset.y));
        draw_scene(picture_canvas, K_PICTURE_SIZE);
        let offset_picture = recorder.finish_recording_as_picture(None);

        self.shaders = TILES
            .iter()
            .map(|&(x, y, w, h, offset_x, offset_y)| {
                let tile = Rect::from_xywh(
                    x * K_PICTURE_SIZE,
                    y * K_PICTURE_SIZE,
                    w * K_PICTURE_SIZE,
                    h * K_PICTURE_SIZE,
                );
                let mut local_matrix = Matrix::new_identity();
                local_matrix.set_translate((offset_x * K_PICTURE_SIZE, offset_y * K_PICTURE_SIZE));
                local_matrix.post_scale(
                    (
                        K_FILL_SIZE / (2.0 * K_PICTURE_SIZE),
                        K_FILL_SIZE / (2.0 * K_PICTURE_SIZE),
                    ),
                    None,
                );
                // When the tile == picture bounds, exercise the picture + offset path.
                let (picture_ref, tile_ptr) =
                    if tile == Rect::from_wh(K_PICTURE_SIZE, K_PICTURE_SIZE) {
                        (&offset_picture, None)
                    } else {
                        (&picture, Some(tile))
                    };
                let picture_ref: Option<&Picture> = picture_ref.as_ref();
                picture_ref.and_then(|picture| {
                    picture.to_shader(
                        (TileMode::Repeat, TileMode::Repeat),
                        FilterMode::Nearest,
                        &local_matrix,
                        tile_ptr.as_ref(),
                    )
                })
            })
            .collect();
    }

    // Port of: gm/pictureshadertile.cpp#L126-L148 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::BLACK);
        let mut paint = Paint::default();
        paint.set_style(Style::Fill);
        for (i, shader) in self.shaders.iter().enumerate() {
            paint.set_shader(shader.clone());
            canvas.save();
            #[allow(clippy::cast_precision_loss)] // mirrors the unsigned-to-float C++ arithmetic
            canvas.translate((
                (i % K_ROW_SIZE) as scalar * K_FILL_SIZE * 1.1,
                (i / K_ROW_SIZE) as scalar * K_FILL_SIZE * 1.1,
            ));
            canvas.draw_rect(Rect::from_wh(K_FILL_SIZE, K_FILL_SIZE), &paint);
            canvas.restore();
        }
    }
}

// Port of: gm/pictureshadertile.cpp#L161-L161 (chrome/m156), DEF_GM(return new PictureShaderTileGM;)
crate::def_gm!(
    #[ignore = "565 mismatch: see notes/gm_pictureshadertile_cpp_PictureShaderTileGM.md"]
    PictureShaderTileGM,
    PictureShaderTileGm::default()
);
