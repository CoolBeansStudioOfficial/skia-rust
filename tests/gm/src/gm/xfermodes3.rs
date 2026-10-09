// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/xfermodes3.cpp (chrome/m156)

// The int-to-scalar casts of small constants and the byte casts of packed channels mirror the C++
// arithmetic of the GM.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
// Single-letter and similar names mirror the C++ GM (w, h, x, y; rect, rrect).
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use crate::prelude::*;
use crate::tool_utils::color_to_565;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::{Color4f, colors};
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surface::Surface;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/xfermodes3.cpp (chrome/m156), enum { kCheckSize = 8, kSize = 30, kTestsPerRow = 15 }
const CHECK_SIZE: i32 = 8;
const SIZE: i32 = 30;
const TESTS_PER_ROW: i32 = 15;

// Port of: gm/xfermodes3.cpp (chrome/m156), class Xfermodes3GM
#[derive(Debug, Default)]
pub struct Xfermodes3Gm {
    bg_shader: Option<Shader>,
    bmp_shader: Option<Shader>,
}

impl Xfermodes3Gm {
    // Port of: gm/xfermodes3.cpp (chrome/m156), makeTempSurface
    // (the GrContext's full-RT optimizations do not apply to a raster surface)
    fn make_temp_surface(base_canvas: &Canvas, w: i32, h: i32) -> Option<Surface<'static>> {
        let base_info = base_canvas.image_info();
        let info = ImageInfo::new(
            (w, h),
            base_info.color_type(),
            base_info.alpha_type(),
            base_info.color_space(),
        );
        base_canvas.new_surface(&info, None)
    }

    // Port of: gm/xfermodes3.cpp (chrome/m156), drawMode
    #[allow(clippy::too_many_arguments)] // mirrors drawMode(canvas, x, y, w, h, paint, surface)
    fn draw_mode(
        &self,
        canvas: &Canvas,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        mode_paint: &Paint,
        surface: Option<&mut Surface<'static>>,
    ) {
        canvas.save();
        canvas.translate((x as scalar, y as scalar));

        let mut r = Rect::from_wh(w as scalar, h as scalar);

        if let Some(surf) = surface {
            let mode_canvas = surf.canvas();
            self.draw_mode_body(mode_canvas, &r, mode_paint);
            surf.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);
        } else {
            canvas.save_layer(&SaveLayerRec::default().bounds(&r));
            canvas.clip_rect(r, None, None);
            self.draw_mode_body(canvas, &r, mode_paint);
            canvas.restore();
        }

        r.inset((-0.5, -0.5));
        let mut border_paint = Paint::default();
        border_paint.set_style(Style::Stroke);
        canvas.draw_rect(r, &border_paint);

        canvas.restore();
    }

    // The two draws both branches of drawMode make onto the mode canvas.
    fn draw_mode_body(&self, mode_canvas: &Canvas, r: &Rect, mode_paint: &Paint) {
        let mut bg_paint = Paint::default();
        bg_paint.set_anti_alias(false);
        bg_paint.set_shader(self.bg_shader.clone());
        mode_canvas.draw_rect(*r, &bg_paint);
        mode_canvas.draw_rect(*r, mode_paint);
    }
}

impl GM for Xfermodes3Gm {
    fn name(&self) -> String {
        "xfermodes3".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(630, 1215)
    }

    fn bg_color(&self) -> Color {
        color_to_565(0xFF70_D0E0)
    }

    // Port of: gm/xfermodes3.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((10.0, 20.0));

        let font = default_portable_font();
        let label_p = Paint::default();

        let k_solid_colors = [Color::TRANSPARENT, Color::BLUE, Color::new(0x8080_8000)];

        let k_bmp_alphas: [u8; 2] = [0xff, 0x80];

        let mut temp_surface = Self::make_temp_surface(canvas, SIZE, SIZE);

        let mut test: i32 = 0;
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        // (style, stroke width)
        let k_strokes: [(Style, scalar); 2] =
            [(Style::Fill, 0.0), (Style::Stroke, SIZE as scalar / 2.0)];
        for (style, width) in k_strokes {
            for &mode in &BlendMode::VALUES {
                canvas.draw_str(
                    mode.name(),
                    (x as scalar, (y + SIZE + 3) as scalar + font.size()),
                    &font,
                    &label_p,
                );
                for &solid in &k_solid_colors {
                    let mut mode_paint = Paint::default();
                    mode_paint.set_blend_mode(mode);
                    mode_paint.set_color(solid);
                    mode_paint.set_style(style);
                    mode_paint.set_stroke_width(width);

                    self.draw_mode(canvas, x, y, SIZE, SIZE, &mode_paint, temp_surface.as_mut());

                    test += 1;
                    x += SIZE + 10;
                    if test % TESTS_PER_ROW == 0 {
                        x = 0;
                        y += SIZE + 30;
                    }
                }
                for &alpha in &k_bmp_alphas {
                    let mut mode_paint = Paint::default();
                    mode_paint.set_blend_mode(mode);
                    mode_paint.set_alpha(alpha);
                    mode_paint.set_shader(self.bmp_shader.clone());
                    mode_paint.set_style(style);
                    mode_paint.set_stroke_width(width);

                    self.draw_mode(canvas, x, y, SIZE, SIZE, &mode_paint, temp_surface.as_mut());

                    test += 1;
                    x += SIZE + 10;
                    if test % TESTS_PER_ROW == 0 {
                        x = 0;
                        y += SIZE + 30;
                    }
                }
            }
        }
    }

    // Port of: gm/xfermodes3.cpp (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let check_data = [
            pack_argb32(0xFF, 0x42, 0x41, 0x42),
            pack_argb32(0xFF, 0xD6, 0xD3, 0xD6),
            pack_argb32(0xFF, 0xD6, 0xD3, 0xD6),
            pack_argb32(0xFF, 0x42, 0x41, 0x42),
        ];
        let mut bg = Bitmap::new();
        bg.alloc_n32_pixels((2, 2), true);
        for (i, &v) in check_data.iter().enumerate() {
            bg.set_addr32((i % 2) as i32, (i / 2) as i32, v);
        }

        let lm = Matrix::scale((CHECK_SIZE as scalar, CHECK_SIZE as scalar));
        self.bg_shader = bg.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            &lm,
        );

        let center = Point::new(SIZE as scalar / 2.0, SIZE as scalar / 2.0);
        let k_colors = [
            Color4f::new(0.0, 0.0, 0.0, 0.0),
            Color4f::from_color(Color::new(0x8080_0000)),
            Color4f::from_color(Color::new(0xF020_F060)),
            colors::WHITE,
        ];
        let grad = Gradient::new(
            Colors::new(&k_colors, None, TileMode::Repeat, None),
            Interpolation::default(),
        );
        let radial = shaders::radial_gradient((center, 3.0 * SIZE as scalar / 4.0), &grad, None);
        let mut bmp_paint = Paint::default();
        bmp_paint.set_shader(radial);

        let mut bmp = Bitmap::new();
        bmp.alloc_n32_pixels((SIZE, SIZE), None);
        {
            let bmp_canvas = Canvas::from_bitmap(&mut bmp, None).expect("a canvas on the bitmap");
            bmp_canvas.clear(Color::TRANSPARENT);
            let rect = Rect::from_ltrb(
                SIZE as scalar / 8.0,
                SIZE as scalar / 8.0,
                7.0 * SIZE as scalar / 8.0,
                7.0 * SIZE as scalar / 8.0,
            );
            bmp_canvas.draw_rect(rect, &bmp_paint);
        }

        self.bmp_shader = bmp.to_shader(None, SamplingOptions::default(), None);
    }
}

// Port of: gm/xfermodes3.cpp (chrome/m156), DEF_GM(return new Xfermodes3GM;)
crate::def_gm!(Xfermodes3GM, Xfermodes3Gm::default());
