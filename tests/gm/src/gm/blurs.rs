// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurs.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::font::Font;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar_round_to_scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blend, blur, color_filter};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/blurs.cpp#L12-L63 (chrome/m156), blurs (DEF_SIMPLE_GM_BG, 0xFFDDDDDD)
crate::def_simple_gm_bg!(blurs, canvas, 700, 500, Color::from(0xFFDD_DDDD), {
    // SkBlurStyle NONE = SkBlurStyle(-999): no mask filter.
    let recs: [(Option<BlurStyle>, i32, i32); 5] = [
        (None, 0, 0),
        (Some(BlurStyle::Inner), -1, 0),
        (Some(BlurStyle::Normal), 0, 1),
        (Some(BlurStyle::Solid), 0, -1),
        (Some(BlurStyle::Outer), 1, 0),
    ];

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::BLUE);

    canvas.translate((-40.0, 0.0));

    for (style, cx, cy) in recs {
        if let Some(style) = style {
            paint.set_mask_filter(MaskFilter::blur(
                style,
                BlurMask::convert_radius_to_sigma(20.0),
                None,
            ));
        } else {
            paint.set_mask_filter(None);
        }
        canvas.draw_circle(
            (int_to_scalar(200 + cx * 100), int_to_scalar(200 + cy * 100)),
            50.0,
            &paint,
        );
    }
    // draw text
    {
        let font = Font::from_size(default_portable_typeface(), 25.0);
        paint.set_mask_filter(MaskFilter::blur(
            BlurStyle::Normal,
            BlurMask::convert_radius_to_sigma(4.0),
            None,
        ));
        let mut x: f32 = 70.0;
        let mut y: f32 = 400.0;
        paint.set_color(Color::BLACK);
        canvas.draw_str("Hamburgefons Style", (x, y), &font, &paint);
        canvas.draw_str("Hamburgefons Style", (x, y + 50.0), &font, &paint);
        paint.set_mask_filter(None);
        paint.set_color(Color::WHITE);
        x -= 2.0;
        y -= 2.0;
        canvas.draw_str("Hamburgefons Style", (x, y), &font, &paint);
    }
});

//////////////////////////////////////////////////////////////////////////////////////////////

// exercise a special-case of blurs, which is two nested rects. These are drawn specially,
// and possibly cached.
//
// in particular, we want to notice that the 2nd rect draws slightly differently, since it
// is translated a fractional amount.
//
// Port of: gm/blurs.cpp#L90-L111 (chrome/m156)
crate::def_simple_gm!(blur2rects, canvas, 700, 500, {
    let mut paint = Paint::default();

    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 2.3, None));

    let outer = Rect::from_xywh(10.125, 10.125, 100.125, 100.0);
    let inner = Rect::from_xywh(20.25, 20.125, 80.0, 80.0);
    let path = PathBuilder::new()
        .add_rect(outer, PathDirection::CW, None)
        .add_rect(inner, PathDirection::CCW, None)
        .detach();

    canvas.draw_path(&path, &paint);
    // important to translate by a factional amount to exercise a different "phase"
    // of the same path w.r.t. the pixel grid
    let dx = scalar_round_to_scalar(path.bounds().width()) + 14.0 + 0.25;
    canvas.translate((dx, 0.0));
    canvas.draw_path(&path, &paint);
});

// Port of: gm/blurs.cpp#L113-L133 (chrome/m156)
crate::def_simple_gm!(blur2rectsnonninepatch, canvas, 700, 500, {
    let mut paint = Paint::default();
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 4.3, None));

    let outer = Rect::from_xywh(10.0, 110.0, 100.0, 100.0);
    let inner = Rect::from_xywh(50.0, 150.0, 10.0, 10.0);
    let path = PathBuilder::new()
        .add_rect(outer, PathDirection::CW, None)
        .add_rect(inner, PathDirection::CW, None)
        .detach();
    canvas.draw_path(&path, &paint);

    let dx = scalar_round_to_scalar(path.bounds().width()) + 40.0 + 0.25;
    canvas.translate((dx, 0.0));
    canvas.draw_path(&path, &paint);

    // Translate to outside of clip bounds.
    canvas.translate((-dx, 0.0));
    canvas.translate((-30.0, -150.0));
    canvas.draw_path(&path, &paint);
});

// Port of: gm/blurs.cpp#L135-L142 (chrome/m156), BlurDrawImage
crate::def_simple_gm!(BlurDrawImage, canvas, 256, 256, {
    let mut paint = Paint::default();
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 10.0, None));
    canvas.clear(Color::from(0xFF88_FF88));
    if let Some(image) = get_resource_as_image("images/mandrill_512_q075.jpg") {
        canvas.scale((0.25, 0.25));
        canvas.draw_image_with_sampling_options(
            &image,
            (256.0, 256.0),
            SamplingOptions::default(),
            Some(&paint),
        );
    }
});

// Port of: gm/blurs.cpp#L144-L149 (chrome/m156), BlurBigSigma. The C++ `layerPaint` is unused.
crate::def_simple_gm!(BlurBigSigma, canvas, 1024, 1024, {
    let mut p = Paint::default();

    p.set_image_filter(blur(500.0, 500.0, TileMode::Decal, None, None));

    canvas.draw_rect(Rect::new(0.0, 0.0, 700.0, 800.0), &p);
});

// Port of: gm/blurs.cpp#L151-L167 (chrome/m156), BlurSmallSigma
crate::def_simple_gm!(BlurSmallSigma, canvas, 512, 256, {
    {
        // Normal sigma on x-axis, a small but non-zero sigma on y-axis that should
        // be treated as identity.
        let mut paint = Paint::default();
        paint.set_image_filter(blur(16.0, 1e-5, TileMode::Decal, None, None));
        canvas.draw_rect(Rect::new(64.0, 64.0, 192.0, 192.0), &paint);
    }

    {
        // Small sigma on both axes, should be treated as identity and no red should show
        let mut paint = Paint::default();
        paint.set_color(Color::RED);
        let rect = Rect::new(320.0, 64.0, 448.0, 192.0);
        canvas.draw_rect(rect, &paint);
        paint.set_color(Color::BLACK);
        paint.set_image_filter(blur(1e-5, 1e-5, TileMode::Decal, None, None));
        canvas.draw_rect(rect, &paint);
    }
});

// Modeled after crbug.com/1500021, incorporates manual tiling to emulate Chrome's raster tiles
// or the tiled rendering mode in Viewer.
// Port of: gm/blurs.cpp#L169-L203 (chrome/m156), TiledBlurBigSigma
crate::def_simple_gm!(TiledBlurBigSigma, canvas, 1024, 768, {
    const TILE_WIDTH: i32 = 342;
    const TILE_HEIGHT: i32 = 256;

    let orig_ctm = canvas.local_to_device();

    for y in 0..3 {
        for x in 0..3 {
            // Define tiled grid in the canvas pixel space
            canvas.save();
            canvas.reset_matrix();

            canvas.clip_irect(
                IRect::from_xywh(x * TILE_WIDTH, y * TILE_HEIGHT, TILE_WIDTH, TILE_HEIGHT),
                None,
            );
            canvas.set_matrix(&orig_ctm);

            let flood = color_filter(
                color_filters::blend(Color4f::from(Color::BLACK), None, BlendMode::Src),
                None,
                None,
            );
            let blend_filter = blend(BlendMode::SrcOver, flood, None, None);
            let blur_filter = blur(206.0, 206.0, TileMode::Decal, blend_filter, None);

            let mut p = Paint::default();
            p.set_image_filter(blur_filter);

            canvas.clip_rect(Rect::new(0.0, 0.0, 1970.0, 1223.0), None, None);
            canvas.save_layer(&SaveLayerRec::default().paint(&p));
            let mut fill = Paint::default();
            fill.set_color(Color::BLUE);
            canvas.draw_circle((600.0, 150.0), 350.0, &fill);
            canvas.restore();
            canvas.restore();
        }
    }
});
