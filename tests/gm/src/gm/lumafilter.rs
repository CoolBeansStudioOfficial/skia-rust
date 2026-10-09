// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lumafilter.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::font::Edging;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::luma_color_filter;
use skia_rust_tools::font_tool_utils::default_portable_font;

const K_SIZE: f32 = 80.0;
const K_INSET: f32 = 10.0;

// Port of: gm/lumafilter.cpp#L10-L11 (chrome/m156), kColor1 / kColor2
fn k_color1() -> Color4f {
    Color4f::new(1.0, 1.0, 0.0, 1.0)
}

fn k_color2() -> Color4f {
    Color4f::new(0x82 as f32 / 255.0, 1.0, 0.0, 1.0)
}

// Port of: gm/lumafilter.cpp#L14-L22 (chrome/m156), draw_label
fn draw_label(canvas: &Canvas, label: &str, offset: Point) {
    let mut font = default_portable_font();
    font.set_edging(Edging::Alias);
    let (width, _) = font.measure_text(label.as_bytes(), TextEncoding::UTF8, None);
    canvas.draw_simple_text(
        label.as_bytes(),
        TextEncoding::UTF8,
        (offset.x - width / 2.0, offset.y),
        &font,
        &Paint::default(),
    );
}

// Port of: gm/lumafilter.cpp#L24-L60 (chrome/m156), draw_scene
fn draw_scene(
    canvas: &Canvas,
    filter: Option<&ColorFilter>,
    mode: BlendMode,
    s1: Option<&Shader>,
    s2: Option<&Shader>,
) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let bounds = Rect::from_wh(K_SIZE, K_SIZE);
    let c = Rect::from_ltrb(
        bounds.left(),
        bounds.top(),
        bounds.center_x(),
        bounds.bottom(),
    );
    paint.set_argb(0x20, 0, 0, 0xff);
    canvas.draw_rect(bounds, &paint);
    canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
    let mut r = bounds;
    r.inset((K_INSET, 0.0));
    paint.set_shader(s1.cloned());
    if s1.is_some() {
        paint.set_color4f(Color4f::from(Color::BLACK), None);
    } else {
        paint.set_color4f(k_color1().with_alpha_byte(0x80), None);
    }
    canvas.draw_oval(r, &paint);
    if s1.is_none() {
        canvas.save();
        canvas.clip_rect(c, None, None);
        paint.set_color4f(k_color1(), None);
        canvas.draw_oval(r, &paint);
        canvas.restore();
    }

    let mut xfer_paint = Paint::default();
    xfer_paint.set_blend_mode(mode);
    canvas.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&xfer_paint));
    let mut r = bounds;
    r.inset((0.0, K_INSET));
    paint.set_shader(s2.cloned());
    if s2.is_some() {
        paint.set_color4f(Color4f::from(Color::BLACK), None);
    } else {
        paint.set_color4f(k_color2().with_alpha_byte(0x80), None);
    }
    paint.set_color_filter(filter.cloned());
    canvas.draw_oval(r, &paint);
    if s2.is_none() {
        canvas.save();
        canvas.clip_rect(c, None, None);
        paint.set_color4f(k_color2(), None);
        canvas.draw_oval(r, &paint);
        canvas.restore();
    }
    canvas.restore();
    canvas.restore();
}

// Port of: gm/lumafilter.cpp#L62-L121 (chrome/m156), LumaFilterGM
struct LumaFilterGm {
    filter: Option<ColorFilter>,
    gr1: Option<Shader>,
    gr2: Option<Shader>,
}

impl GM for LumaFilterGm {
    fn name(&self) -> String {
        "lumafilter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(600, 420)
    }

    // Port of: gm/lumafilter.cpp#L64-L73 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let g1_colors = [k_color1(), k_color1().with_alpha_byte(0x20)];
        let g2_colors = [k_color2(), k_color2().with_alpha_byte(0x20)];
        let g1_points = [Point::new(0.0, 0.0), Point::new(0.0, 100.0)];
        let g2_points = [Point::new(0.0, 0.0), Point::new(K_SIZE, 0.0)];
        let pos: [f32; 2] = [0.2, 1.0];
        self.filter = luma_color_filter::make();
        self.gr1 = gradient_shaders::linear_gradient(
            (g1_points[0], g1_points[1]),
            &Gradient::new(
                Colors::new(&g1_colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
        self.gr2 = gradient_shaders::linear_gradient(
            (g2_points[0], g2_points[1]),
            &Gradient::new(
                Colors::new(&g2_colors, Some(&pos), TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        );
    }

    // Port of: gm/lumafilter.cpp#L86-L118 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let modes = [
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::SrcATop,
            BlendMode::DstATop,
            BlendMode::SrcIn,
            BlendMode::DstIn,
        ];
        let shaders: [(Option<&Shader>, Option<&Shader>); 4] = [
            (None, None),
            (None, self.gr2.as_ref()),
            (self.gr1.as_ref(), None),
            (self.gr1.as_ref(), self.gr2.as_ref()),
        ];
        let grid_step = K_SIZE + 2.0 * K_INSET;
        for (i, mode) in modes.iter().enumerate() {
            draw_label(
                canvas,
                mode.name(),
                Point::new(grid_step * (0.5 + i as f32), 20.0),
            );
        }
        for (i, (s1, s2)) in shaders.iter().enumerate() {
            canvas.save();
            canvas.translate((K_INSET, grid_step * i as f32 + 30.0));
            for mode in modes {
                draw_scene(canvas, self.filter.as_ref(), mode, *s1, *s2);
                canvas.translate((grid_step, 0.0));
            }
            canvas.restore();
        }
    }
}

// Port of: gm/lumafilter.cpp#L166 (chrome/m156), DEF_GM(return new LumaFilterGM;)
crate::def_gm!(
    LumaFilterGM,
    LumaFilterGm {
        filter: None,
        gr1: None,
        gr2: None
    }
);
