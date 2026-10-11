// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dropshadowimagefilter.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::font::Font;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_effects::image_filters::{color_filter, drop_shadow, drop_shadow_only};
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/dropshadowimagefilter.cpp#L33-L41 (chrome/m156), draw_paint
fn draw_paint_proc(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    paint.set_color(Color::BLACK);
    canvas.save();
    canvas.clip_rect(r, None, None);
    canvas.draw_paint(&paint);
    canvas.restore();
}

// Port of: gm/dropshadowimagefilter.cpp#L43-L52 (chrome/m156), draw_path
fn draw_path_proc(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_color(Color::GREEN);
    paint.set_image_filter(imf);
    paint.set_anti_alias(true);
    canvas.save();
    canvas.clip_rect(r, None, None);
    canvas.draw_circle((r.center_x(), r.center_y()), r.width() / 3.0, &paint);
    canvas.restore();
}

// Port of: gm/dropshadowimagefilter.cpp#L54-L65 (chrome/m156), draw_text
fn draw_text_proc(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    paint.set_color(Color::GREEN);
    paint.set_anti_alias(true);
    let font = Font::from_size(default_portable_typeface(), r.height() / 2.0);
    canvas.save();
    canvas.clip_rect(r, None, None);
    draw_string(
        canvas,
        "Text",
        r.center_x(),
        r.center_y(),
        &font,
        &paint,
        Align::Center,
    );
    canvas.restore();
}

// Port of: gm/dropshadowimagefilter.cpp#L67-L78 (chrome/m156), draw_bitmap
fn draw_bitmap_proc(canvas: &Canvas, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    let bounds: IRect = r.round_out();
    let info = ImageInfo::new_n32((bounds.width(), bounds.height()), AlphaType::Premul, None);
    let mut surf = surfaces::raster(&info, None, None).expect("raster surface");
    draw_path_proc(surf.canvas(), r, None);
    paint.set_image_filter(imf);
    canvas.save();
    canvas.clip_rect(r, None, None);
    surf.draw(canvas, (0.0, 0.0), SamplingOptions::default(), Some(&paint));
    canvas.restore();
}

// Port of: gm/dropshadowimagefilter.cpp#L85-L127 (chrome/m156), dropshadowimagefilter
crate::def_simple_gm!(dropshadowimagefilter, canvas, 400, 656, {
    let draw_proc: [fn(&Canvas, Rect, Option<ImageFilter>); 4] = [
        draw_bitmap_proc,
        draw_path_proc,
        draw_paint_proc,
        draw_text_proc,
    ];

    let cf = color_filters::blend(Color4f::from_color(Color::MAGENTA), None, BlendMode::SrcIn);
    let cfif = color_filter(cf, None, None);
    let crop_rect = IRect::from_xywh(10, 10, 44, 44);
    let bogus_rect = IRect::from_xywh(-100, -100, 10, 10);

    let spin_cs = ColorSpace::new_srgb().make_color_spin();
    let blue = Color4f::from_color(Color::BLUE);
    let filters: [Option<ImageFilter>; 8] = [
        None,
        drop_shadow((7.0, 0.0), (0.0, 3.0), blue, None, None, None),
        drop_shadow((0.0, 7.0), (3.0, 0.0), blue, None, None, None),
        drop_shadow((7.0, 7.0), (3.0, 3.0), blue, None, None, None),
        drop_shadow((7.0, 7.0), (3.0, 3.0), blue, None, cfif, None),
        drop_shadow(
            (7.0, 7.0),
            (3.0, 3.0),
            Color4f::from_color(Color::GREEN),
            Some(&spin_cs),
            None,
            Some(Rect::from_irect(crop_rect)),
        ),
        drop_shadow(
            (7.0, 7.0),
            (3.0, 3.0),
            blue,
            None,
            None,
            Some(Rect::from_irect(bogus_rect)),
        ),
        drop_shadow_only((7.0, 7.0), (3.0, 3.0), blue, None, None, None),
    ];

    let r = Rect::from_wh(64.0, 64.0);
    let margin: f32 = 16.0;
    let dx = r.width() + margin;
    let dy = r.height() + margin;

    canvas.translate((margin, margin));
    for proc_ in draw_proc {
        canvas.save();
        for imf in &filters {
            proc_(canvas, r, imf.clone());
            canvas.translate((0.0, dy));
        }
        canvas.restore();
        canvas.translate((dx, 0.0));
    }
});

// Port of: gm/dropshadowimagefilter.cpp#L140-L182 (chrome/m156)
crate::def_simple_gm!(dropshadow_pseudopersp, canvas, 155, 155, {
    canvas.clear(Color::LIGHT_GRAY);
    canvas.concat_44(&M44::new(
        0.5, 0.0, 0.0, -75.0, 0.0, 0.5, 0.0, -30.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ));
    // This 4x4 matrix technically has perspective, but it only impacts the Z values and the
    // projection doesn't appear to have any distortion. However, the projected coordinates have
    // Z values very different from 0. When inversing the device bounds with an assumed Z=0, the
    // layer bounds end up empty. This GM ensures layer mapping calculations don't discard it.
    canvas.concat_44(&M44::new(
        1360.0, 0.0, 275.4, 294_100.0, 0.0, 1360.0, 489.6, 98_344.0, 0.0, 0.0, -0.51, -2180.67,
        0.0, 0.0, 0.51, 2181.67,
    ));

    let layer_bounds = Rect::new(42.5, 42.5, 457.5, 457.5);
    let mut layer_paint = Paint::default();
    layer_paint.set_image_filter(drop_shadow(
        (30.0, 30.0),
        (12.0, 12.0),
        Color4f::new(0.14902, 0.215_686, 0.329_412, 0.666_667),
        None,
        None,
        None,
    ));
    canvas.save_layer(
        &SaveLayerRec::default()
            .bounds(&layer_bounds)
            .paint(&layer_paint),
    );

    let rrect = RRect::new_rect_xy(Rect::new(-250.0, -250.0, 250.0, 250.0), 45.0, 45.0);
    let mut rrect_paint = Paint::default();
    rrect_paint.set_color(Color::WHITE);
    rrect_paint.set_anti_alias(true);

    canvas.concat_44(&M44::new(
        0.83, 0.0, 0.0, 250.0, 0.0, 0.83, 0.0, 250.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ));
    canvas.draw_rrect(rrect, &rrect_paint);
    canvas.restore();

    canvas.concat_44(&M44::new(
        0.83, 0.0, 0.0, 250.0, 0.0, 0.83, 0.0, 250.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ));

    rrect_paint.set_color(Color::BLACK);
    rrect_paint.set_style(Style::Stroke);
    canvas.draw_rrect(rrect, &rrect_paint);
});
