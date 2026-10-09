// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefiltersbase.cpp (chrome/m156)
//
// Only `ImageFiltersBaseGM` is ported here. The `textfilter_*` GMs of this file have their own
// manifest entries.

// GM ports mirror the C++ integer and scalar casts.
#![allow(clippy::cast_precision_loss, clippy::too_many_lines)]

use crate::prelude::*;
use crate::tool_utils::{int_to_scalar, make_surface};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::canvas::{AutoCanvasRestore, Canvas as CoreCanvas};
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::Edging;
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::scalar::degrees_to_radians;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::text_utils::{self, Align};
use skia_rust_effects::image_filters;
use skia_rust_effects::image_filters::{blur, color_filter, drop_shadow, empty, offset};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/imagefiltersbase.cpp#L14-L23 (chrome/m156), draw_paint
fn draw_paint(canvas: &Canvas, _image: &Image, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    paint.set_color(Color::GREEN);
    canvas.save();
    canvas.clip_rect(r, None, None);
    canvas.draw_paint(&paint);
    canvas.restore();
}

// Port of: gm/imagefiltersbase.cpp#L25-L32 (chrome/m156), draw_line
fn draw_line(canvas: &Canvas, _image: &Image, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_color(Color::BLUE);
    paint.set_image_filter(imf);
    paint.set_stroke_width(r.width() / 10.0);
    canvas.draw_line((r.left(), r.top()), (r.right(), r.bottom()), &paint);
}

// Port of: gm/imagefiltersbase.cpp#L34-L41 (chrome/m156), draw_rect
fn draw_rect(canvas: &Canvas, _image: &Image, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_color(Color::YELLOW);
    paint.set_image_filter(imf);
    let mut rr = r;
    rr.inset((r.width() / 10.0, r.height() / 10.0));
    canvas.draw_rect(rr, &paint);
}

// Port of: gm/imagefiltersbase.cpp#L43-L50 (chrome/m156), draw_path
fn draw_path(canvas: &Canvas, _image: &Image, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_color(Color::MAGENTA);
    paint.set_image_filter(imf);
    paint.set_anti_alias(true);
    canvas.draw_circle((r.center_x(), r.center_y()), r.width() * 2.0 / 5.0, &paint);
}

// Port of: gm/imagefiltersbase.cpp#L52-L59 (chrome/m156), draw_text
fn draw_text(canvas: &Canvas, _image: &Image, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    paint.set_color(Color::CYAN);
    let font = Font::from_size(default_portable_typeface(), r.height() / 2.0);
    text_utils::draw_string(
        canvas,
        "Text",
        r.center_x(),
        r.center_y(),
        &font,
        &paint,
        Align::Center,
    );
}

// Port of: gm/imagefiltersbase.cpp#L61-L72 (chrome/m156), draw_bitmap
fn draw_bitmap(canvas: &Canvas, image: &Image, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    let bounds: IRect = RoundOut::<IRect>::round_out(&r);
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((bounds.width(), bounds.height()), None);
    bm.erase_color(Color::TRANSPARENT);
    {
        let c = CoreCanvas::from_bitmap(&mut bm, None).expect("a canvas on the bitmap");
        draw_path(&c, image, r, None);
    }
    if let Some(bitmap_image) = bm.as_image() {
        canvas.draw_image_with_sampling_options(
            &bitmap_image,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&paint),
        );
    }
}

// Port of: gm/imagefiltersbase.cpp#L74-L98 (chrome/m156), draw_patch
fn draw_patch(canvas: &Canvas, _image: &Image, r: Rect, imf: Option<ImageFilter>) {
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    // The order of the colors and points is clockwise starting at upper-left corner.
    let cubics: [Point; 12] = [
        //top points
        Point::new(100.0, 100.0),
        Point::new(150.0, 50.0),
        Point::new(250.0, 150.0),
        Point::new(300.0, 100.0),
        //right points
        Point::new(250.0, 150.0),
        Point::new(350.0, 250.0),
        //bottom points
        Point::new(300.0, 300.0),
        Point::new(250.0, 250.0),
        Point::new(150.0, 350.0),
        Point::new(100.0, 300.0),
        //left points
        Point::new(50.0, 250.0),
        Point::new(150.0, 150.0),
    ];
    let colors: [Color; 4] = [Color::RED, Color::GREEN, Color::BLUE, Color::CYAN];
    let _acr = AutoCanvasRestore::guard(canvas, true);
    canvas.translate((-r.left(), -r.top()));
    canvas.scale((r.width() / 400.0, r.height() / 400.0));
    canvas.draw_patch(&cubics, Some(&colors), None, BlendMode::Dst, &paint);
}

// Port of: gm/imagefiltersbase.cpp#L100-L109 (chrome/m156), draw_atlas
fn draw_atlas(canvas: &Canvas, atlas: &Image, r: Rect, imf: Option<ImageFilter>) {
    let rad = degrees_to_radians(15.0);
    let xform = RSXform::new(rad.cos(), rad.sin(), (r.width() * 0.15, 0.0));
    let mut paint = Paint::default();
    paint.set_image_filter(imf);
    paint.set_anti_alias(true);
    let sampling = SamplingOptions::from(CubicResampler::mitchell());
    canvas.draw_atlas(
        atlas,
        &[xform],
        &[r],
        None,
        BlendMode::Src,
        sampling,
        None,
        Some(&paint),
    );
}

// Port of: gm/imagefiltersbase.cpp#L111-L121 (chrome/m156), create_atlas_image
fn create_atlas_image(canvas: &Canvas) -> Image {
    let size_w: f32 = 64.0;
    let size_h: f32 = 64.0;
    let atlas_info = ImageInfo::new((64, 64), ColorType::N32, AlphaType::Premul, None);
    let mut atlas_surface = make_surface(canvas, &atlas_info, None).expect("a surface");
    {
        let atlas_canvas = atlas_surface.canvas();
        let mut atlas_paint = Paint::default();
        atlas_paint.set_color(Color::GRAY);
        let font = Font::from_size(default_portable_typeface(), size_h * 0.4);
        text_utils::draw_string(
            atlas_canvas,
            "Atlas",
            size_w * 0.5,
            size_h * 0.5,
            &font,
            &atlas_paint,
            Align::Center,
        );
    }
    atlas_surface.image_snapshot().expect("a snapshot")
}

// Port of: gm/imagefiltersbase.cpp#L123-L128 (chrome/m156), draw_frame
fn draw_frame(canvas: &Canvas, r: Rect) {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_color(Color::RED);
    canvas.draw_rect(r, &paint);
}

type DrawProc = fn(&Canvas, &Image, Rect, Option<ImageFilter>);

// Port of: gm/imagefiltersbase.cpp#L129-L193 (chrome/m156), ImageFiltersBaseGM
struct ImageFiltersBaseGm {
    atlas: Option<Image>,
}

impl GM for ImageFiltersBaseGm {
    fn name(&self) -> String {
        "imagefiltersbase".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(700, 500)
    }

    // Port of: gm/imagefiltersbase.cpp#L134-L190 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        if self.atlas.is_none() {
            self.atlas = Some(create_atlas_image(canvas));
        }
        let atlas = self.atlas.clone().expect("an atlas image");
        let draw_proc: [DrawProc; 8] = [
            draw_paint,
            draw_line,
            draw_rect,
            draw_path,
            draw_text,
            draw_bitmap,
            draw_patch,
            draw_atlas,
        ];
        let cf = color_filters::blend(Color4f::from(Color::RED), None, BlendMode::SrcIn);
        let filters: [Option<ImageFilter>; 6] = [
            None,
            // "identity"
            offset((0.0, 0.0), None, None),
            Some(empty()),
            color_filter(cf, None, None),
            // The strange 0.29 value tickles an edge case where crop rect calculates
            // a small border, but the blur really needs no border. This tickles
            // an msan uninitialized value bug.
            blur(12.0, 0.29, TileMode::Decal, None, None),
            drop_shadow(
                (10.0, 5.0),
                (3.0, 3.0),
                Color4f::from(Color::BLUE),
                None,
                None,
                None,
            ),
        ];
        let r = Rect::new(0.0, 0.0, int_to_scalar(64), int_to_scalar(64));
        let margin = int_to_scalar(16);
        let dx = r.width() + margin;
        let dy = r.height() + margin;
        canvas.translate((margin, margin));
        for proc_fn in draw_proc {
            canvas.save();
            for filter in &filters {
                proc_fn(canvas, &atlas, r, filter.clone());
                draw_frame(canvas, r);
                canvas.translate((0.0, dy));
            }
            canvas.restore();
            canvas.translate((dx, 0.0));
        }
    }
}

// Port of: gm/imagefiltersbase.cpp#L193 (chrome/m156), DEF_GM( return new ImageFiltersBaseGM; )
crate::def_gm!(ImageFiltersBaseGM, ImageFiltersBaseGm { atlas: None });

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
