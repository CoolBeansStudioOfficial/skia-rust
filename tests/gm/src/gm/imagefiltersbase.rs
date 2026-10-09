// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersbase.cpp (chrome/m156)
//
// Only the text variants are ported here (`ImageFiltersText_IF` and `ImageFiltersText_CF`, with
// their base `ImageFiltersTextBaseGM`). `ImageFiltersBaseGM` has its own manifest entry.

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{AutoCanvasRestore, SaveLayerRec};
use skia_rust_core::color_filters;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::paint::Paint;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/imagefiltersbase.cpp#L233-L240 (chrome/m156), the filter installed by a subclass
#[derive(Clone, Copy)]
enum FilterKind {
    // Port of: gm/imagefiltersbase.cpp#L301-L307 (chrome/m156), ImageFiltersText_IF::installFilter
    Image,
    // Port of: gm/imagefiltersbase.cpp#L311-L317 (chrome/m156), ImageFiltersText_CF::installFilter
    Color,
}

// Port of: gm/imagefiltersbase.cpp#L233-L290 (chrome/m156), ImageFiltersTextBaseGM
struct ImageFiltersTextGm {
    suffix: &'static str,
    kind: FilterKind,
}

impl ImageFiltersTextGm {
    // Port of: gm/imagefiltersbase.cpp#L301-L303 (chrome/m156), ImageFiltersText_IF constructor
    fn new_image() -> Self {
        Self {
            suffix: "image",
            kind: FilterKind::Image,
        }
    }

    // Port of: gm/imagefiltersbase.cpp#L311-L313 (chrome/m156), ImageFiltersText_CF constructor
    fn new_color() -> Self {
        Self {
            suffix: "color",
            kind: FilterKind::Color,
        }
    }

    // Port of: gm/imagefiltersbase.cpp#L301-L307 and #L311-L317 (chrome/m156), installFilter
    fn install_filter(&self, paint: &mut Paint) {
        match self.kind {
            FilterKind::Image => {
                paint.set_image_filter(image_filters::blur(1.5, 1.5, TileMode::Decal, None, None));
            }
            FilterKind::Color => {
                paint.set_color_filter(color_filters::blend(
                    Color4f::from(Color::BLUE),
                    None,
                    BlendMode::SrcIn,
                ));
            }
        }
    }

    // Port of: gm/imagefiltersbase.cpp#L247-L259 (chrome/m156), drawWaterfall
    fn draw_waterfall(canvas: &Canvas, paint: &Paint) {
        let edgings = [Edging::Alias, Edging::AntiAlias, Edging::SubpixelAntiAlias];
        let mut font = Font::from_size(default_portable_typeface(), 30.0);
        let acr = AutoCanvasRestore::guard(canvas, true);
        for edging in edgings {
            font.set_edging(edging);
            canvas.draw_str("Hamburgefon", (0.0, 0.0), &font, paint);
            canvas.translate((0.0, 40.0));
        }
        acr.restore();
    }
}

impl GM for ImageFiltersTextGm {
    // Port of: gm/imagefiltersbase.cpp#L241-L246 (chrome/m156), getName
    fn name(&self) -> String {
        format!("textfilter_{}", self.suffix)
    }

    // Port of: gm/imagefiltersbase.cpp#L247 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(512, 342)
    }

    // Port of: gm/imagefiltersbase.cpp#L262-L281 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((20.0, 40.0));
        for do_save_layer in [false, true] {
            let acr = AutoCanvasRestore::guard(canvas, true);
            for use_filter in [false, true] {
                let acr2 = AutoCanvasRestore::guard(canvas, true);
                let mut paint = Paint::default();
                if use_filter {
                    self.install_filter(&mut paint);
                }
                if do_save_layer {
                    canvas.save_layer(&SaveLayerRec::default().paint(&paint));
                    paint.set_image_filter(None);
                }
                Self::draw_waterfall(canvas, &paint);
                acr2.restore();
                canvas.translate((250.0, 0.0));
            }
            acr.restore();
            canvas.translate((0.0, 200.0));
        }
    }
}

// Port of: gm/imagefiltersbase.cpp#L303 (chrome/m156), ImageFiltersText_IF registration
crate::def_gm!(ImageFiltersText_IF, ImageFiltersTextGm::new_image());
// Port of: gm/imagefiltersbase.cpp#L313 (chrome/m156), ImageFiltersText_CF registration
crate::def_gm!(ImageFiltersText_CF, ImageFiltersTextGm::new_color());
