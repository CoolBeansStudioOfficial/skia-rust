// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_12212.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::Edging;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::text_blob::TextBlob;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/skbug_12212.cpp#L19-L43 (chrome/m156), DEF_SIMPLE_GM_BG(skbug_12212, ..., SK_ColorCYAN)
crate::def_simple_gm_bg!(skbug_12212, canvas, 400, 400, Color::CYAN, {
    // Create an Alpha_8 surface to draw into (strangely, with RGB pixel geometry).
    let image_info = ImageInfo::new((400, 400), ColorType::Alpha8, AlphaType::Premul, None);
    let props = SurfaceProps::new(SurfacePropsFlags::empty(), PixelGeometry::RGBH);
    // The GPU render target is not used on a raster sink, so only the raster surface is made.
    let mut surface = surfaces::raster(&image_info, None, Some(&props)).expect("a raster surface");

    // Draw text into the surface using LCD antialiasing.
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_blend_mode(skia_rust_core::blend_mode::BlendMode::Src);
    p.set_alpha(0x80);
    let mut font = default_portable_font();
    font.set_size(170.0);
    font.set_edging(Edging::SubpixelAntiAlias);
    let text_blob = TextBlob::from_text(b"text", TextEncoding::UTF8, &font).expect("a text blob");
    surface
        .canvas()
        .draw_text_blob(&text_blob, (50.0, 350.0), &p);

    // Draw the surface on our main canvas.
    surface.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);
});
