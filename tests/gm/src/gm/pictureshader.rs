// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pictureshader.cpp (chrome/m156)

//! Picture shaders (`SkPicture::makeShader`) drawn in a grid of local matrices, tile modes and
//! scenes, next to the same scenes drawn with bitmap shaders, plus the tiled and perspective
//! picture shader cases.

use crate::GM;
use crate::canvas::Canvas;
use crate::tool_utils::color_to_565;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::FontHinting;
use skia_rust_core::m44::{M44, V3};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

/// `kTileConfigs`: the tile modes of each scene column.
// Port of: gm/pictureshader.cpp#L28-L32 (chrome/m156)
const K_TILE_CONFIGS: [(TileMode, TileMode); 3] = [
    (TileMode::Repeat, TileMode::Repeat),
    (TileMode::Repeat, TileMode::Clamp),
    (TileMode::Mirror, TileMode::Repeat),
];

// Port of: gm/pictureshader.cpp#L34-L99 (chrome/m156), class PictureShaderGM
struct PictureShaderGm {
    tile_size: scalar,
    scene_size: scalar,
    alpha: f32,
    use_local_matrix_wrapper: bool,
    picture: Option<Picture>,
    bitmap: Bitmap,
}

impl PictureShaderGm {
    // Port of: gm/pictureshader.cpp#L35-L41 (chrome/m156), constructor
    fn new(
        tile_size: scalar,
        scene_size: scalar,
        use_local_matrix_wrapper: bool,
        alpha: f32,
    ) -> Self {
        Self {
            tile_size,
            scene_size,
            alpha,
            use_local_matrix_wrapper,
            picture: None,
            bitmap: Bitmap::new(),
        }
    }

    // Port of: gm/pictureshader.cpp#L120-L179 (chrome/m156), drawSceneColumn
    fn draw_scene_column(
        &self,
        canvas: &Canvas,
        pos: (scalar, scalar),
        scale: scalar,
        local_scale: scalar,
        tile_mode: usize,
    ) {
        let mut ctm = Matrix::new_identity();
        let mut local_matrix = Matrix::new_identity();

        ctm.set_translate(pos);
        ctm.pre_scale((scale, scale), None);
        local_matrix.set_scale((local_scale, local_scale), None);
        self.draw_scene(canvas, &ctm, &local_matrix, tile_mode);

        ctm.set_translate((pos.0, pos.1 + self.scene_size * 1.2 * scale));
        ctm.pre_scale((scale, scale), None);
        local_matrix.set_translate((self.tile_size / 4.0, self.tile_size / 4.0));
        local_matrix.pre_scale((local_scale, local_scale), None);
        self.draw_scene(canvas, &ctm, &local_matrix, tile_mode);

        ctm.set_translate((pos.0, pos.1 + self.scene_size * 2.4 * scale));
        ctm.pre_scale((scale, scale), None);
        local_matrix.set_rotate(45.0, None);
        local_matrix.pre_scale((local_scale, local_scale), None);
        self.draw_scene(canvas, &ctm, &local_matrix, tile_mode);

        ctm.set_translate((pos.0, pos.1 + self.scene_size * 3.6 * scale));
        ctm.pre_scale((scale, scale), None);
        local_matrix.set_skew((1.0, 0.0), None);
        local_matrix.pre_scale((local_scale, local_scale), None);
        self.draw_scene(canvas, &ctm, &local_matrix, tile_mode);

        ctm.set_translate((pos.0, pos.1 + self.scene_size * 4.8 * scale));
        ctm.pre_scale((scale, scale), None);
        local_matrix.set_translate((self.tile_size / 4.0, self.tile_size / 4.0));
        local_matrix.pre_rotate(45.0, None);
        local_matrix.pre_scale((local_scale, local_scale), None);
        self.draw_scene(canvas, &ctm, &local_matrix, tile_mode);
    }

    // Port of: gm/pictureshader.cpp#L181-L193 (chrome/m156), drawTile
    fn draw_tile(&self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_color(Color::GREEN);
        paint.set_style(Style::Fill);
        paint.set_anti_alias(true);

        let t = self.tile_size;
        canvas.draw_circle((t / 4.0, t / 4.0), t / 4.0, &paint);
        canvas.draw_rect(Rect::from_xywh(t / 2.0, t / 2.0, t / 2.0, t / 2.0), &paint);

        paint.set_color(Color::RED);
        canvas.draw_line((t / 2.0, t * 1.0 / 3.0), (t / 2.0, t * 2.0 / 3.0), &paint);
        canvas.draw_line((t * 1.0 / 3.0, t / 2.0), (t * 2.0 / 3.0, t / 2.0), &paint);
    }

    // Port of: gm/pictureshader.cpp#L195-L229 (chrome/m156), drawScene
    fn draw_scene(
        &self,
        canvas: &Canvas,
        matrix: &Matrix,
        local_matrix: &Matrix,
        tile_mode: usize,
    ) {
        let (tmx, tmy) = K_TILE_CONFIGS[tile_mode];
        let Some(picture) = self.picture.as_ref() else {
            return;
        };

        let mut paint = Paint::default();
        paint.set_style(Style::Fill);
        paint.set_color(Color::new(0xFFCC_CCCC));

        canvas.save();
        canvas.concat(matrix);
        canvas.draw_rect(Rect::from_wh(self.scene_size, self.scene_size), &paint);
        canvas.draw_rect(
            Rect::from_xywh(self.scene_size * 1.1, 0.0, self.scene_size, self.scene_size),
            &paint,
        );

        paint.set_alpha_f(self.alpha);

        let picture_shader = picture.to_shader(
            (tmx, tmy),
            FilterMode::Nearest,
            if self.use_local_matrix_wrapper {
                None
            } else {
                Some(local_matrix)
            },
            None,
        );
        let picture_shader = if self.use_local_matrix_wrapper {
            picture_shader.map(|s| s.with_local_matrix(local_matrix))
        } else {
            picture_shader
        };
        paint.set_shader(picture_shader);
        canvas.draw_rect(Rect::from_wh(self.scene_size, self.scene_size), &paint);

        canvas.translate((self.scene_size * 1.1, 0.0));

        let bitmap_shader = self.bitmap.to_shader(
            (tmx, tmy),
            SamplingOptions::default(),
            if self.use_local_matrix_wrapper {
                None
            } else {
                Some(local_matrix)
            },
        );
        let bitmap_shader: Option<Shader> = if self.use_local_matrix_wrapper {
            bitmap_shader.map(|s| s.with_local_matrix(local_matrix))
        } else {
            bitmap_shader
        };
        paint.set_shader(bitmap_shader);
        canvas.draw_rect(Rect::from_wh(self.scene_size, self.scene_size), &paint);

        canvas.restore();
    }
}

impl GM for PictureShaderGm {
    // Port of: gm/pictureshader.cpp#L105-L109 (chrome/m156), getName
    fn name(&self) -> String {
        format!(
            "pictureshader{}{}",
            if self.use_local_matrix_wrapper {
                "_localwrapper"
            } else {
                ""
            },
            if self.alpha < 1.0 { "_alpha" } else { "" }
        )
    }

    // Port of: gm/pictureshader.cpp#L111-L111 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1400, 1450)
    }

    // Port of: gm/pictureshader.cpp#L51-L68 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // Build the picture.
        let mut recorder = PictureRecorder::new();
        let picture_canvas =
            recorder.begin_recording(Rect::from_wh(self.tile_size, self.tile_size), false);
        self.draw_tile(picture_canvas);
        self.picture = recorder.finish_recording_as_picture(None);

        // Build a reference bitmap.
        // `SkScalarCeilToInt`: the tile size is a small integer, exact in i32.
        #[allow(clippy::cast_possible_truncation)]
        let dim = ISize::new(self.tile_size.ceil() as i32, self.tile_size.ceil() as i32);
        let mut bitmap = Bitmap::new();
        bitmap.alloc_n32_pixels(dim, None);
        bitmap.erase_color(Color::TRANSPARENT);
        {
            let bitmap_canvas =
                Canvas::from_bitmap(&mut bitmap, None).expect("a canvas on the reference bitmap");
            self.draw_tile(&bitmap_canvas);
        }
        self.bitmap = bitmap;
    }

    // Port of: gm/pictureshader.cpp#L123-L135 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let s = self.scene_size;
        self.draw_scene_column(canvas, (0.0, 0.0), 1.0, 1.0, 0);
        self.draw_scene_column(canvas, (0.0, s * 6.4), 1.0, 2.0, 0);
        self.draw_scene_column(canvas, (s * 2.4, 0.0), 1.0, 1.0, 1);
        self.draw_scene_column(canvas, (s * 2.4, s * 6.4), 1.0, 1.0, 2);
        self.draw_scene_column(canvas, (s * 4.8, 0.0), 2.0, 1.0, 0);
        self.draw_scene_column(canvas, (s * 9.6, 0.0), 2.0, 2.0, 0);

        // One last custom row to exercise negative scaling
        let mut ctm = Matrix::new_identity();
        let mut local_matrix = Matrix::new_identity();
        ctm.set_translate((s * 2.1, s * 13.8));
        ctm.pre_scale((-1.0, -1.0), None);
        local_matrix.set_scale((2.0, 2.0), None);
        self.draw_scene(canvas, &ctm, &local_matrix, 0);

        ctm.set_translate((s * 2.4, s * 12.8));
        local_matrix.set_scale((-1.0, -1.0), None);
        self.draw_scene(canvas, &ctm, &local_matrix, 0);

        ctm.set_translate((s * 4.8, s * 12.3));
        ctm.pre_scale((2.0, 2.0), None);
        self.draw_scene(canvas, &ctm, &local_matrix, 0);

        ctm.set_translate((s * 13.8, s * 14.3));
        ctm.pre_scale((-2.0, -2.0), None);
        local_matrix.set_translate((self.tile_size / 4.0, self.tile_size / 4.0));
        local_matrix.pre_rotate(45.0, None);
        local_matrix.pre_scale((-2.0, -2.0), None);
        self.draw_scene(canvas, &ctm, &local_matrix, 0);
    }
}

// Port of: gm/pictureshader.cpp#L205-L207 (chrome/m156), DEF_GM registrations
crate::def_gm!(
    PictureShaderGm_50_100 = "PictureShaderGM(50, 100)",
    PictureShaderGm::new(50.0, 100.0, false, 1.0)
);
crate::def_gm!(
    PictureShaderGm_50_100_true = "PictureShaderGM(50, 100, true)",
    PictureShaderGm::new(50.0, 100.0, true, 1.0)
);
crate::def_gm!(
    PictureShaderGm_50_100_false_025 = "PictureShaderGM(50, 100, false, 0.25f)",
    PictureShaderGm::new(50.0, 100.0, false, 0.25)
);

// Port of: gm/pictureshader.cpp#L209-L237 (chrome/m156), tiled_picture_shader
crate::def_simple_gm!(tiled_picture_shader, canvas, 400, 400, {
    // https://code.google.com/p/skia/issues/detail?id=3398
    let tile = Rect::from_wh(100.0, 100.0);

    let mut recorder = PictureRecorder::new();
    let c = recorder.begin_recording(tile, false);

    let mut r = tile;
    r.inset((4.0, 4.0));
    let mut p = Paint::default();
    p.set_color(color_to_565(0xFF30_3F9F)); // dark blue
    c.draw_rect(r, &p);
    p.set_color(color_to_565(0xFFC5_CAE9)); // light blue
    p.set_stroke_width(10.0);
    c.draw_line((20.0, 20.0), (80.0, 80.0), &p);

    let picture = recorder.finish_recording_as_picture(None);

    p.set_color(color_to_565(0xFF8B_C34A)); // green
    canvas.draw_paint(&p);

    canvas.clip_rect(Rect::from_xywh(0.0, 0.0, 400.0, 350.0), None, None);
    p.set_color(Color::new(0xFFB6_B6B6)); // gray
    canvas.draw_paint(&p);

    if let Some(picture) = picture {
        p.set_shader(picture.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            FilterMode::Nearest,
            None,
            None,
        ));
    }
    canvas.draw_paint(&p);
});

// Port of: gm/pictureshader.cpp#L239-L306 (chrome/m156), pictureshader_persp
crate::def_simple_gm!(
    #[ignore = "f16 mismatch from host libm tanf (glibc vs UCRT): see notes/gm_pictureshader_cpp_pictureshader_persp.md"]
    pictureshader_persp,
    canvas,
    215,
    110,
    {
        // `DrawStrategy`.
        #[derive(Clone, Copy)]
        enum DrawStrategy {
            Direct,
            PictureShader,
        }

        let draw_picture = |canvas: &Canvas, picture: &Picture, strategy: DrawStrategy| {
            // Only want local upper 50x50 of 'picture' before we apply decal (or clip)
            let bounds = Rect::from_ltrb(0.0, 0.0, 50.0, 50.0);
            match strategy {
                DrawStrategy::Direct => {
                    canvas.clip_rect(bounds, None, true);
                    canvas.draw_picture(picture, None, None);
                }
                DrawStrategy::PictureShader => {
                    let mut paint = Paint::default();
                    paint.set_shader(picture.to_shader(
                        (TileMode::Decal, TileMode::Decal),
                        FilterMode::Linear,
                        None,
                        &bounds,
                    ));
                    canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 50.0, 50.0), &paint);
                }
            }
        };

        let picture = {
            let mut font = Font::from_typeface(Some(default_portable_typeface()));
            font.set_size(8.0);
            font.set_hinting(FontHinting::Normal);

            let mut paint = Paint::default();
            paint.set_color(Color::GREEN);
            let mut recorder = PictureRecorder::new();
            let record_canvas =
                recorder.begin_recording(Rect::from_ltrb(0.0, 0.0, 100.0, 100.0), false);
            if let Some(blob) = TextBlob::from_str("Hamburgefons", &font) {
                record_canvas.draw_text_blob(&blob, (0.0, 16.0), &paint);
            }
            recorder.finish_recording_as_picture(None)
        };
        let Some(picture) = picture else {
            return;
        };

        let mut m = M44::default();
        m.pre_scale(2.0, 2.0);
        let mut persp = M44::perspective(0.01, 10.0, std::f32::consts::PI / 3.0);
        persp.pre_translate(0.0, 5.0, -0.1);
        persp.pre_concat(&M44::rotate(V3::new(0.0, 1.0, 0.0), 0.008));
        m.post_concat(&persp);

        canvas.clear(Color::BLACK);
        canvas.translate((5.0, 5.0));
        for strategy in [DrawStrategy::Direct, DrawStrategy::PictureShader] {
            canvas.save();

            let mut outline = Paint::default();
            outline.set_color(Color::WHITE);
            outline.set_style(Style::Stroke);
            outline.set_stroke_width(1.0);
            canvas.draw_rect(Rect::from_ltrb(-1.0, -1.0, 101.0, 101.0), &outline);

            canvas.clip_rect(Rect::from_ltrb(0.0, 0.0, 100.0, 100.0), None, None);
            canvas.concat_44(&m);

            draw_picture(canvas, &picture, strategy);
            canvas.restore();

            canvas.translate((105.0, 0.0));
        }
    }
);
