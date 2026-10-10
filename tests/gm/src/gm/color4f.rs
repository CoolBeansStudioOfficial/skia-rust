// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/color4f.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_raster::surfaces;

// Port of: gm/color4f.cpp#L9-L11 (chrome/m156)
fn make_opaque_color() -> Shader {
    shaders::color(Color::new(0xFFFF0000))
}

// Port of: gm/color4f.cpp#L12-L14 (chrome/m156)
fn make_alpha_color() -> Shader {
    shaders::color(Color::new(0x80FF0000))
}

// Port of: gm/color4f.cpp#L15-L17 (chrome/m156)
fn make_cf_null() -> Option<ColorFilter> {
    None
}

// Port of: gm/color4f.cpp#L18-L22 (chrome/m156)
fn make_cf0() -> Option<ColorFilter> {
    let mut cm = ColorMatrix::default();
    cm.set_saturation(0.75);
    color_filters::matrix(&cm, Clamp::Yes)
}

// Port of: gm/color4f.cpp#L23-L28 (chrome/m156)
fn make_cf1() -> Option<ColorFilter> {
    let mut cm = ColorMatrix::default();
    cm.set_saturation(0.75);
    let a = color_filters::matrix(&cm, Clamp::Yes)?;
    cm.set_scale(1.1, 0.9, 1.0, 1.0);
    Some(a.composed(color_filters::matrix(&cm, Clamp::Yes)))
}

// Port of: gm/color4f.cpp#L29-L31 (chrome/m156)
fn make_cf2() -> Option<ColorFilter> {
    color_filters::blend(
        Color4f::from_color(Color::new(0x8044CC88)),
        None,
        BlendMode::SrcATop,
    )
}

// Port of: gm/color4f.cpp#L32-L44 (chrome/m156)
fn draw_into_canvas(canvas: &Canvas) {
    let r = Rect::from_wh(50.0, 100.0);
    let shaders: [fn() -> Shader; 2] = [make_opaque_color, make_alpha_color];
    let filters: [fn() -> Option<ColorFilter>; 4] = [make_cf_null, make_cf0, make_cf1, make_cf2];
    let mut paint = Paint::default();
    for sh_proc in shaders {
        paint.set_shader(sh_proc());
        for cf_proc in filters {
            paint.set_color_filter(cf_proc());
            canvas.draw_rect(r, &paint);
            canvas.translate((60.0, 0.0));
        }
    }
}

// Port of: gm/color4f.cpp#L46-L62 (chrome/m156)
crate::def_simple_gm!(color4f, canvas, 1024, 260, {
    canvas.translate((10.0, 10.0));
    let mut bg = Paint::default();
    bg.set_color(Color::new(0xFFFFFFFF));
    let color_spaces: [Option<ColorSpace>; 2] = [None, Some(ColorSpace::new_srgb())];
    for color_space in color_spaces {
        let info = ImageInfo::new_n32_premul((1024, 100), color_space);
        let mut surface = surfaces::raster(&info, None, None).expect("a surface");
        surface.canvas().draw_paint(&bg);
        draw_into_canvas(surface.canvas());
        surface.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);
        canvas.translate((0.0, 120.0));
    }
});

// Port of: gm/color4f.cpp#L97-L129 (chrome/m156)
crate::def_simple_gm!(color4shader, canvas, 360, 480, {
    canvas.translate((10.0, 10.0));
    let srgb = ColorSpace::new_srgb();
    let spin = srgb.with_color_spin(); // RGB -> GBR
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(0.5, 0.5, 0.5, 1.0),
    ];
    let mut paint = Paint::default();
    let r = Rect::from_wh(100.0, 100.0);
    for c4 in colors {
        let shaders_list = [
            shaders::color_in_space(c4, None),
            shaders::color_in_space(c4, srgb.clone()),
            shaders::color_in_space(c4, spin.clone()),
        ];
        canvas.save();
        for s in shaders_list {
            paint.set_shader(s);
            canvas.draw_rect(r, &paint);
            canvas.translate((r.width() * 6.0 / 5.0, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, r.height() * 6.0 / 5.0));
    }
});

// Port of: gm/color4f.cpp#L131-L166 (chrome/m156)
crate::def_simple_gm!(color4blendcf, canvas, 360, 480, {
    canvas.translate((10.0, 10.0));
    let srgb = ColorSpace::new_srgb();
    let spin = srgb.with_color_spin(); // RGB -> GBR
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(0.5, 0.5, 0.5, 1.0),
    ];
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    let r = Rect::from_wh(100.0, 100.0);
    for c4 in colors {
        // Use kModulate and a paint color of white so the final drawn color is color-space
        // managed 'c4'.
        let filters = [
            color_filters::blend(c4, None, BlendMode::Modulate),
            color_filters::blend(c4, Some(&srgb), BlendMode::Modulate),
            color_filters::blend(c4, Some(&spin), BlendMode::Modulate),
        ];
        canvas.save();
        for f in filters {
            paint.set_color_filter(f);
            canvas.draw_rect(r, &paint);
            canvas.translate((r.width() * 6.0 / 5.0, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, r.height() * 6.0 / 5.0));
    }
});
