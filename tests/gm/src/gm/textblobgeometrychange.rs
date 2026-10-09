// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobgeometrychange.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_typeface};

// This tests that we don't try to reuse textblobs from the GPU textblob cache across pixel geometry
// changes when we have LCD.  crbug/486744
const WIDTH: i32 = 200;
const HEIGHT: i32 = 200;

// Port of: gm/textblobgeometrychange.cpp#L14-L70 (chrome/m156), TextBlobGeometryChange
struct TextBlobGeometryChangeGm;

impl GM for TextBlobGeometryChangeGm {
    // Port of: gm/textblobgeometrychange.cpp#L20 (chrome/m156), getName
    fn name(&self) -> String {
        "textblobgeometrychange".to_owned()
    }

    // Port of: gm/textblobgeometrychange.cpp#L22 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/textblobgeometrychange.cpp#L24-L53 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let text = "Hamburgefons";

        let mut font = Font::from_size(default_portable_typeface(), 20.0);
        font.set_edging(Edging::SubpixelAntiAlias);

        let mut builder = TextBlobBuilder::new();
        add_to_text_blob(&mut builder, text, &font, 10.0, 10.0);
        let blob = builder.make().expect("the text has glyphs");

        let info = ImageInfo::new((200, 200), ColorType::N32, AlphaType::Premul, None);
        let props = SurfaceProps::new(SurfacePropsFlags::default(), PixelGeometry::Unknown);
        let mut surface = tool_utils::make_surface(canvas, &info, Some(&props)).expect("a surface");
        let c = surface.canvas();

        // LCD text on white background
        let rect = Rect::from_ltrb(0.0, 0.0, int_to_scalar(WIDTH), int_to_scalar(HEIGHT) / 2.0);
        let mut rect_paint = Paint::default();
        rect_paint.set_color(Color::from(0xffff_ffff_u32));
        canvas.draw_rect(rect, &rect_paint);
        canvas.draw_text_blob(&blob, (10.0, 50.0), &Paint::default());

        // This should not look garbled since we should disable LCD text in this case
        // (i.e., unknown pixel geometry)
        c.clear(Color::from(0x00ff_ffff_u32));
        c.draw_text_blob(&blob, (10.0, 150.0), &Paint::default());
        surface.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);
    }
}

// Port of: gm/textblobgeometrychange.cpp#L69 (chrome/m156)
crate::def_gm!(TextBlobGeometryChange, TextBlobGeometryChangeGm);
