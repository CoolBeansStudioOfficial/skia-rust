// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/modecolorfilters.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{color_to_565, int_to_scalar};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SaveLayerRec};
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::raster_canvas::RasterCanvas;

const WIDTH: i32 = 512;
const HEIGHT: i32 = 1024;

// Using gradients because GPU doesn't currently have an implementation of SkColorShader (duh!)
// Port of: gm/modecolorfilters.cpp#L17-L22 (chrome/m156), make_color_shader
fn make_color_shader(color: Color) -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
    let c4 = Color4f::from(color);
    let colors = [c4, c4];
    shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/modecolorfilters.cpp#L24-L26 (chrome/m156), make_solid_shader
fn make_solid_shader() -> Option<Shader> {
    make_color_shader(Color::from_argb(0xFF, 0x42, 0x82, 0x21))
}

// Port of: gm/modecolorfilters.cpp#L28-L30 (chrome/m156), make_transparent_shader
fn make_transparent_shader() -> Option<Shader> {
    make_color_shader(Color::from_argb(0x80, 0x10, 0x70, 0x20))
}

// Port of: gm/modecolorfilters.cpp#L32-L34 (chrome/m156), make_trans_black_shader
fn make_trans_black_shader() -> Option<Shader> {
    make_color_shader(Color::from(0x0))
}

// draws a background behind each test rect to see transparency
// Port of: gm/modecolorfilters.cpp#L36-L53 (chrome/m156), make_bg_shader
fn make_bg_shader(check_size: i32) -> Option<Shader> {
    let mut bmp = Bitmap::new();
    bmp.alloc_n32_pixels((2 * check_size, 2 * check_size), None);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bmp, None).expect("a canvas on the bitmap");
        canvas.clear(color_to_565(0xFF80_0000));
        let mut paint = Paint::default();
        paint.set_color(color_to_565(0xFF00_0080));
        let rect0 = Rect::from_xywh(
            0.0,
            0.0,
            int_to_scalar(check_size),
            int_to_scalar(check_size),
        );
        let rect1 = Rect::from_xywh(
            int_to_scalar(check_size),
            int_to_scalar(check_size),
            int_to_scalar(check_size),
            int_to_scalar(check_size),
        );
        canvas.draw_rect(rect1, &paint);
        canvas.draw_rect(rect0, &paint);
    }
    bmp.as_image()?.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        None,
    )
}

// Port of: gm/modecolorfilters.cpp#L55-L152 (chrome/m156), ModeColorFilterGM
struct ModeColorFilterGm {
    bmp_shader: Option<Shader>,
}

impl ModeColorFilterGm {
    fn new() -> Self {
        Self { bmp_shader: None }
    }
}

impl GM for ModeColorFilterGm {
    fn name(&self) -> String {
        "modecolorfilters".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFF30_3030)
    }

    // Port of: gm/modecolorfilters.cpp#L70-L151 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // size of rect for each test case
        const RECT_WIDTH: i32 = 20;
        const RECT_HEIGHT: i32 = 20;
        const CHECK_SIZE: i32 = 10;

        if self.bmp_shader.is_none() {
            self.bmp_shader = make_bg_shader(CHECK_SIZE);
        }

        let mut bg_paint = Paint::default();
        bg_paint.set_shader(self.bmp_shader.clone());
        bg_paint.set_blend_mode(BlendMode::Src);

        let shaders_list: [Option<Shader>; 4] = [
            // use a paint color instead of a shader
            None,
            make_solid_shader(),
            make_transparent_shader(),
            make_trans_black_shader(),
        ];

        // used without shader
        let colors: [Color; 5] = [
            Color::from_argb(0xFF, 0xFF, 0xFF, 0xFF),
            Color::from_argb(0xFF, 0x00, 0x00, 0x00),
            Color::from_argb(0x00, 0x00, 0x00, 0x00),
            Color::from_argb(0xFF, 0x10, 0x20, 0x42),
            Color::from_argb(0xA0, 0x20, 0x30, 0x90),
        ];

        // used with shaders
        let alphas: [Color; 2] = [Color::from(0xFFFF_FFFF), Color::from(0x8080_8080)];
        // currently just doing the Modes expressible as Coeffs
        let modes: [BlendMode; 14] = [
            BlendMode::Clear,
            BlendMode::Src,
            BlendMode::Dst,
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcATop,
            BlendMode::DstATop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Modulate,
        ];

        let mut paint = Paint::default();
        let mut idx: i32 = 0;
        let rects_per_row = std::cmp::max(WIDTH / RECT_WIDTH, 1);
        for mode in modes {
            for color in colors {
                paint.set_color_filter(color_filters::blend(Color4f::from(color), None, mode));
                for shader in &shaders_list {
                    paint.set_shader(shader.clone());
                    let has_shader = paint.shader().is_none();
                    let paint_colors: &[Color] = if has_shader { &alphas } else { &colors };
                    for &paint_color in paint_colors {
                        paint.set_color(paint_color);
                        let x = int_to_scalar(idx % rects_per_row);
                        let y = int_to_scalar(idx / rects_per_row);
                        let rect = Rect::from_xywh(
                            x * int_to_scalar(RECT_WIDTH),
                            y * int_to_scalar(RECT_HEIGHT),
                            int_to_scalar(RECT_WIDTH),
                            int_to_scalar(RECT_HEIGHT),
                        );
                        canvas.save_layer(&SaveLayerRec::default().bounds(&rect));
                        canvas.draw_rect(rect, &bg_paint);
                        canvas.draw_rect(rect, &paint);
                        canvas.restore();
                        idx += 1;
                    }
                }
            }
        }
    }
}

// Port of: gm/modecolorfilters.cpp#L154 (chrome/m156)
crate::def_gm!(ModeColorFilterGM, ModeColorFilterGm::new());
