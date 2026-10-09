// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lcdblendmodes.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{create_checkerboard_shader, int_to_scalar, make_surface};
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const K_COL_WIDTH: i32 = 180;
const K_NUM_COLS: i32 = 4;
const K_WIDTH: i32 = K_COL_WIDTH * K_NUM_COLS;
const K_HEIGHT: i32 = 750;

// Port of: gm/lcdblendmodes.cpp#L13-L23 (chrome/m156), make_shader
fn make_shader(bounds: Rect) -> Option<Shader> {
    let pts = [
        Point::new(bounds.left(), bounds.top()),
        Point::new(bounds.right(), bounds.bottom()),
    ];
    let colors = [Color4f::from(Color::RED), Color4f::from(Color::GREEN)];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Repeat, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/lcdblendmodes.cpp#L25-L100 (chrome/m156), LcdBlendGM
struct LcdBlendGm {
    text_height: f32,
    checkerboard: Option<Shader>,
}

impl LcdBlendGm {
    // Port of: gm/lcdblendmodes.cpp#L27-L30 (chrome/m156), the constructor
    fn new() -> Self {
        const K_POINT_SIZE: i32 = 25;
        Self {
            text_height: int_to_scalar(K_POINT_SIZE),
            checkerboard: None,
        }
    }

    // Port of: gm/lcdblendmodes.cpp#L56-L95 (chrome/m156), drawColumn
    fn draw_column(
        &self,
        canvas: &Canvas,
        background_color: Color,
        text_color: Color,
        use_grad: bool,
    ) {
        const G_MODES: [BlendMode; 29] = [
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
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Multiply,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ];
        // Draw background rect
        let mut background_paint = Paint::default();
        background_paint.set_color(background_color);
        canvas.draw_rect(
            Rect::new(
                0.0,
                0.0,
                int_to_scalar(K_COL_WIDTH),
                int_to_scalar(K_HEIGHT),
            ),
            &background_paint,
        );

        let mut y = self.text_height;
        for mode in G_MODES {
            let mut paint = Paint::default();
            paint.set_color(text_color);
            paint.set_blend_mode(mode);
            let mut font = Font::from_size(default_portable_typeface(), self.text_height);
            font.set_subpixel(true);
            font.set_edging(Edging::SubpixelAntiAlias);
            if use_grad {
                let r = Rect::new(0.0, y - self.text_height, int_to_scalar(K_COL_WIDTH), y);
                paint.set_shader(make_shader(r));
            }
            canvas.draw_str(mode.name(), (0.0, y), &font, &paint);
            y += self.text_height;
        }
    }
}

impl GM for LcdBlendGm {
    fn name(&self) -> String {
        "lcdblendmodes".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(K_WIDTH, K_HEIGHT)
    }

    // Port of: gm/lcdblendmodes.cpp#L40-L42 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.checkerboard = create_checkerboard_shader(Color::BLACK, Color::WHITE, 4);
    }

    // Port of: gm/lcdblendmodes.cpp#L44-L70 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut p = Paint::default();
        p.set_anti_alias(false);
        p.set_style(Style::Fill);
        p.set_shader(self.checkerboard.clone());
        let r = Rect::new(0.0, 0.0, int_to_scalar(K_WIDTH), int_to_scalar(K_HEIGHT));
        canvas.draw_rect(r, &p);

        let info = ImageInfo::new_n32_premul((K_WIDTH, K_HEIGHT), None);
        let props = SurfaceProps::new(SurfacePropsFlags::default(), PixelGeometry::RGBH);
        let mut surface = make_surface(canvas, &info, Some(&props)).expect("a surface");
        let surf_canvas = surface.canvas();
        self.draw_column(surf_canvas, Color::BLACK, Color::WHITE, false);
        surf_canvas.translate((int_to_scalar(K_COL_WIDTH), 0.0));
        self.draw_column(surf_canvas, Color::WHITE, Color::BLACK, false);
        surf_canvas.translate((int_to_scalar(K_COL_WIDTH), 0.0));
        self.draw_column(surf_canvas, Color::GREEN, Color::MAGENTA, false);
        surf_canvas.translate((int_to_scalar(K_COL_WIDTH), 0.0));
        self.draw_column(surf_canvas, Color::CYAN, Color::MAGENTA, true);

        let mut surf_paint = Paint::default();
        surf_paint.set_blend_mode(BlendMode::SrcOver);
        surface.draw(
            canvas,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&surf_paint),
        );
    }
}

// Port of: gm/lcdblendmodes.cpp#L100 (chrome/m156)
crate::def_gm!(LcdBlendGM, LcdBlendGm::new());
