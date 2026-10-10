// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hsl.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-scalar conversions of small sizes (exact in f32)
#![allow(clippy::float_cmp)] // mirrors the C++ `mx == mn` test of set_sat

// Hue, Saturation, Color, and Luminosity blend modes are oddballs. The reference functions below
// mirror the C++ float arithmetic operation for operation.

use crate::tool_utils::get_resource_as_image;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_priv::ColorConverter;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/hsl.cpp#L22-L23 (chrome/m156)
fn min(r: f32, g: f32, b: f32) -> f32 {
    r.min(g.min(b))
}

// Port of: gm/hsl.cpp#L22-L23 (chrome/m156)
fn max(r: f32, g: f32, b: f32) -> f32 {
    r.max(g.max(b))
}

// Port of: gm/hsl.cpp#L25 (chrome/m156)
fn sat(r: f32, g: f32, b: f32) -> f32 {
    max(r, g, b) - min(r, g, b)
}

// Port of: gm/hsl.cpp#L26 (chrome/m156)
fn lum(r: f32, g: f32, b: f32) -> f32 {
    r * 0.30 + g * 0.59 + b * 0.11
}

// Port of: gm/hsl.cpp#L31-L42 (chrome/m156)
fn set_sat(r: &mut f32, g: &mut f32, b: &mut f32, s: f32) {
    let mn = min(*r, *g, *b);
    let mx = max(*r, *g, *b);
    let channel = |c: f32| {
        if mx == mn {
            0.0
        } else {
            (c - mn) * s / (mx - mn)
        }
    };
    *r = channel(*r);
    *g = channel(*g);
    *b = channel(*b);
}

// Port of: gm/hsl.cpp#L43-L58 (chrome/m156)
fn clip_color(r: &mut f32, g: &mut f32, b: &mut f32) {
    let l = lum(*r, *g, *b);
    let mn = min(*r, *g, *b);
    let mx = max(*r, *g, *b);
    let clip = |mut c: f32| {
        if mn < 0.0 {
            c = l + (c - l) * l / (l - mn);
        }
        if mx > 1.0 {
            c = l + (c - l) * (1.0 - l) / (mx - l);
        }
        // SkASSERT(-0.0001f < c) and SkASSERT(c <= 1) are debug-only checks in the C++.
        c
    };
    *r = clip(*r);
    *g = clip(*g);
    *b = clip(*b);
}

// Port of: gm/hsl.cpp#L59-L66 (chrome/m156)
fn set_lum(r: &mut f32, g: &mut f32, b: &mut f32, l: f32) {
    let diff = l - lum(*r, *g, *b);
    *r += diff;
    *g += diff;
    *b += diff;
    clip_color(r, g, b);
}

// Port of: gm/hsl.cpp#L69-L80 (chrome/m156)
fn hue(dr: f32, dg: f32, db: f32, sr: &mut f32, sg: &mut f32, sb: &mut f32) {
    // Hue of Src, Saturation and Luminosity of Dst.
    let (mut r, mut g, mut b) = (*sr, *sg, *sb);
    set_sat(&mut r, &mut g, &mut b, sat(dr, dg, db));
    set_lum(&mut r, &mut g, &mut b, lum(dr, dg, db));
    *sr = r;
    *sg = g;
    *sb = b;
}

// Port of: gm/hsl.cpp#L82-L93 (chrome/m156)
fn saturation(dr: f32, dg: f32, db: f32, sr: &mut f32, sg: &mut f32, sb: &mut f32) {
    // Saturation of Src, Hue and Luminosity of Dst
    let (mut r, mut g, mut b) = (dr, dg, db);
    set_sat(&mut r, &mut g, &mut b, sat(*sr, *sg, *sb));
    set_lum(&mut r, &mut g, &mut b, lum(dr, dg, db)); // This may seem redundant, but it is not.
    *sr = r;
    *sg = g;
    *sb = b;
}

// Port of: gm/hsl.cpp#L95-L105 (chrome/m156)
fn color(dr: f32, dg: f32, db: f32, sr: &mut f32, sg: &mut f32, sb: &mut f32) {
    // Hue and Saturation of Src, Luminosity of Dst.
    let (mut r, mut g, mut b) = (*sr, *sg, *sb);
    set_lum(&mut r, &mut g, &mut b, lum(dr, dg, db));
    *sr = r;
    *sg = g;
    *sb = b;
}

// Port of: gm/hsl.cpp#L107-L117 (chrome/m156)
fn luminosity(dr: f32, dg: f32, db: f32, sr: &mut f32, sg: &mut f32, sb: &mut f32) {
    // Luminosity of Src, Hue and Saturation of Dst.
    let (mut r, mut g, mut b) = (dr, dg, db);
    set_lum(&mut r, &mut g, &mut b, lum(*sr, *sg, *sb));
    *sr = r;
    *sg = g;
    *sb = b;
}

type BlendRef = fn(f32, f32, f32, &mut f32, &mut f32, &mut f32);

// Port of: gm/hsl.cpp#L119-L130 (chrome/m156)
fn blend(dst: Color, src: Color, mode: BlendRef) -> Color {
    let d = Color4f::from_color(dst);
    let mut s = Color4f::from_color(src);

    mode(d.r, d.g, d.b, &mut s.r, &mut s.g, &mut s.b);

    s.to_color()
}

// Port of: gm/hsl.cpp#L132-L170 (chrome/m156)
crate::def_simple_gm!(hsl, canvas, 600, 100, {
    let font = default_portable_font();

    let comment = "HSL blend modes are correct when you see no circles in the squares.";
    canvas.draw_str(comment, (10.0, 10.0), &font, &Paint::default());

    // Just to keep things reaaaal simple, we'll only use opaque colors.
    let mut bg = Paint::default();
    let mut fg = Paint::default();
    bg.set_color(Color::new(0xff00_ff00)); // Fully-saturated bright green, H = 120°, S = 100%, L = 50%.
    fg.set_color(Color::new(0xff7f_3f7f)); // Partly-saturated dim magenta, H = 300°, S = ~33%, L = ~37%.

    let tests: [(BlendMode, Option<BlendRef>); 6] = [
        (BlendMode::Src, None),
        (BlendMode::Dst, None),
        (BlendMode::Hue, Some(hue)),
        (BlendMode::Saturation, Some(saturation)),
        (BlendMode::Color, Some(color)),
        (BlendMode::Luminosity, Some(luminosity)),
    ];
    for (mode, reference) in tests {
        canvas.draw_rect(Rect::from_ltrb(20.0, 20.0, 80.0, 80.0), &bg);

        fg.set_blend_mode(mode);
        canvas.draw_rect(Rect::from_ltrb(20.0, 20.0, 80.0, 80.0), &fg);

        if let Some(reference) = reference {
            let mut reference_paint = Paint::default();
            reference_paint.set_color(blend(bg.color(), fg.color(), reference));
            canvas.draw_circle((50.0, 50.0), 20.0, &reference_paint);
        }

        canvas.draw_str(mode.name(), (20.0, 90.0), &font, &Paint::default());

        canvas.translate((100.0, 0.0));
    }
});

// Trying to match sample images on https://www.w3.org/TR/compositing-1/#blendingnonseparable
// Port of: gm/hsl.cpp#L173-L179 (chrome/m156)
fn make_grad(width: f32) -> skia_rust_core::shader::Shader {
    let pts = [Point::new(0.0, 0.0), Point::new(width, 0.0)];
    let conv = ColorConverter::new(&[
        Color::new(0xFF00_CCCC),
        Color::new(0xFF00_00CC),
        Color::new(0xFFCC_00CC),
        Color::new(0xFFCC_0000),
        Color::new(0xFFCC_CC00),
        Color::new(0xFF00_CC00),
    ]);
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(conv.colors4f(), None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
    .expect("a gradient shader")
}

// Port of: gm/hsl.cpp#L181-L221 (chrome/m156)
crate::def_simple_gm!(HSL_duck, canvas, 1110, 620, {
    let src = get_resource_as_image("images/ducky.png").expect("images/ducky.png");
    let dst = make_grad(src.width() as f32);
    let r = Rect::from_ltrb(0.0, 0.0, src.width() as f32, src.height() as f32);

    canvas.translate((10.0, 50.0));
    canvas.scale((0.5, 0.5));

    let recs = [
        (BlendMode::Hue, "Hue"),
        (BlendMode::Saturation, "Saturation"),
        (BlendMode::Color, "Color"),
        (BlendMode::Luminosity, "Luminosity"),
    ];

    let mut font: Font = default_portable_font();
    font.set_size(40.0);
    font.set_edging(Edging::AntiAlias);

    canvas.save();
    for (_, name) in recs {
        canvas.draw_simple_text(
            name,
            TextEncoding::UTF8,
            (150.0, -20.0),
            &font,
            &Paint::default(),
        );
        canvas.translate((r.width() + 10.0, 0.0));
    }
    canvas.restore();

    for src_a in [1.0_f32, 0.5] {
        canvas.save();
        for (mode, _) in recs {
            let mut p = Paint::default();
            p.set_shader(Some(dst.clone()));
            canvas.draw_rect(r, &p); // bg

            p.set_shader(None);
            p.set_blend_mode(mode);
            p.set_alpha_f(src_a);
            canvas.draw_image_rect_with_sampling_options(
                &src,
                Some((&r, SrcRectConstraint::Strict)),
                r,
                SamplingOptions::default(),
                &p,
            );

            canvas.translate((r.width() + 10.0, 0.0));
        }
        let text = format!("alpha {src_a}");
        canvas.draw_simple_text(
            text.as_bytes(),
            TextEncoding::UTF8,
            (10.0, r.height() / 2.0),
            &font,
            &Paint::default(),
        );

        canvas.restore();
        canvas.translate((0.0, r.height() + 10.0));
    }
});
