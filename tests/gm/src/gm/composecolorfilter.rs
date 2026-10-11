// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/composecolorfilter.cpp (chrome/m156)

//! `composeCF`: a color filter network, built with `makeComposed` and with a runtime color filter
//! that does the same thing.
//!
//! Not ported: `composeCFIF`, which needs the turbulence shader and image filters.

#![allow(clippy::cast_precision_loss)] // the tint matrix is built from 8-bit channels

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{Clamp, matrix_row_major};
use skia_rust_core::data::Data;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeEffect};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters::shader_filter::{self, Dither};
use skia_rust_effects::image_filters::{color_filter, compose};
use skia_rust_effects::luma_color_filter;
use skia_rust_effects::perlin_noise_shader::shaders;

// Port of: gm/composecolorfilter.cpp#L37-L79 (chrome/m156)
fn make_tint_color_filter(lo: Color, hi: Color, use_sk_sl: bool) -> Option<ColorFilter> {
    let r_lo = i32::from(lo.r());
    let g_lo = i32::from(lo.g());
    let b_lo = i32::from(lo.b());
    let a_lo = i32::from(lo.a());
    let r_hi = i32::from(hi.r());
    let g_hi = i32::from(hi.g());
    let b_hi = i32::from(hi.b());
    let a_hi = i32::from(hi.a());

    // We map component-wise:
    //
    //   r' = lo.r + (hi.r - lo.r) * luma
    //   g' = lo.g + (hi.g - lo.g) * luma
    //   b' = lo.b + (hi.b - lo.b) * luma
    //   a' = lo.a + (hi.a - lo.a) * luma
    //
    // The input luminance is stored in the alpha channel
    // (and RGB are cleared -- see SkLumaColorFilter). Thus:
    let tint_matrix: [f32; 20] = [
        0.0,
        0.0,
        0.0,
        (r_hi - r_lo) as f32 / 255.0,
        r_lo as f32 / 255.0,
        0.0,
        0.0,
        0.0,
        (g_hi - g_lo) as f32 / 255.0,
        g_lo as f32 / 255.0,
        0.0,
        0.0,
        0.0,
        (b_hi - b_lo) as f32 / 255.0,
        b_lo as f32 / 255.0,
        0.0,
        0.0,
        0.0,
        (a_hi - a_lo) as f32 / 255.0,
        a_lo as f32 / 255.0,
    ];

    let inner = luma_color_filter::make();
    let outer = matrix_row_major(&tint_matrix, Clamp::Yes);

    // Prove that we can implement compose-color-filter using runtime effects
    if use_sk_sl {
        let effect = RuntimeEffect::make_for_color_filter(
            "uniform colorFilter inner;\
             uniform colorFilter outer;\
             half4 main(half4 c) { return outer.eval(inner.eval(c)); }",
            None,
        )
        .expect("the compose effect compiles");
        let children = [
            inner.map_or(ChildPtr::Empty, ChildPtr::from),
            outer.map_or(ChildPtr::Empty, ChildPtr::from),
        ];
        effect.make_color_filter(Data::new_empty(), &children)
    } else {
        outer.map(|outer| outer.composed(inner))
    }
}

// Port of: gm/composecolorfilter.cpp#L81-L105 (chrome/m156)
crate::def_simple_gm!(composeCF, canvas, 200, 200, {
    // This GM draws a simple color-filter network, using the existing "makeComposed" API, and also
    // using a runtime color filter that does the same thing.
    let mut paint = Paint::default();
    let gradient_colors = [colors::RED, colors::GREEN, colors::BLUE, colors::RED];
    let gradient = Gradient::new(
        Colors::new(&gradient_colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    paint.set_shader(gradient_shaders::sweep_gradient(
        (50.0, 50.0),
        (0.0, 360.0),
        &gradient,
        None,
    ));
    canvas.save();
    for use_sk_sl in [false, true] {
        let cf0 =
            make_tint_color_filter(Color::new(0xff30_0000), Color::new(0xffa0_0000), use_sk_sl); // red tint
        let cf1 =
            make_tint_color_filter(Color::new(0xff00_3000), Color::new(0xff00_a000), use_sk_sl); // green tint
        paint.set_color_filter(cf0);
        canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &paint);
        canvas.translate((100.0, 0.0));
        paint.set_color_filter(cf1);
        canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &paint);
        canvas.restore();
        canvas.translate((0.0, 100.0));
    }
});

// Port of: gm/composecolorfilter.cpp#L107-L149 (chrome/m156), composeCFIF
crate::def_simple_gm!(composeCFIF, canvas, 604, 200, {
    // This GM draws a ::Shader image filter composed with a ::ColorFilter image filter in two
    // ways (direct and via ::Compose). This ensures the use (or non-use in this case) of the source
    // image is the same across both means of composition.
    let cf = make_tint_color_filter(Color::new(0xff30_0000), Color::new(0xffa0_0000), false);
    let shader = shaders::turbulence((0.01, 0.01), 2, 0.0, None).expect("turbulence shader");

    let shader_if = shader_filter::shader(Some(shader.clone()), Dither::No, None);
    let direct_compose = color_filter(cf.clone(), shader_if.clone(), None);
    let indirect_compose = compose(
        // outer = ColorFilter(cf, nullptr), inner = shaderIF
        color_filter(cf.clone(), None, None),
        shader_if,
    );

    {
        // Directly draw the shader composed with the color filter
        canvas.save();
        canvas.clip_rect(Rect::new(0.0, 0.0, 200.0, 200.0), None, None);
        let mut p = Paint::default();
        p.set_shader(shader);
        p.set_color_filter(cf);
        canvas.draw_paint(&p);
        canvas.restore();
    }
    canvas.translate((202.0, 0.0));
    {
        // Draw with the directly composed image filter
        canvas.save();
        canvas.clip_rect(Rect::new(0.0, 0.0, 200.0, 200.0), None, None);
        let mut p = Paint::default();
        p.set_image_filter(direct_compose);
        canvas.draw_paint(&p);
        canvas.restore();
    }
    canvas.translate((202.0, 0.0));
    {
        // Draw with the indirectly composed image filter
        canvas.save();
        canvas.clip_rect(Rect::new(0.0, 0.0, 200.0, 200.0), None, None);
        let mut p = Paint::default();
        p.set_image_filter(indirect_compose);
        canvas.draw_paint(&p);
        canvas.restore();
    }
});
