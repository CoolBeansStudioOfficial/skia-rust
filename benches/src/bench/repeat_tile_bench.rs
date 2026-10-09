// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/RepeatTileBench.cpp

//! `RepeatTileBench`: a 50×50 bitmap (a red disc and a blue frame) used as a repeating shader,
//! filling the canvas with `drawPaint` (a rendering bench).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;

use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::def_bench;
use crate::prelude::*;
use crate::tool_utils::colortype_name;

/// `static void draw_into_bitmap(const SkBitmap& bm)`.
// Port of: bench/RepeatTileBench.cpp#L17-L34 (chrome/m156)
fn draw_into_bitmap(bm: &mut Bitmap) {
    let w = bm.width();
    let h = bm.height();

    // SkCanvas canvas(bm);
    let canvas = Canvas::from_bitmap(bm, None).expect("bitmap ready to draw");
    // SkPaint p;
    let mut p = Paint::default();
    // p.setAntiAlias(true);
    p.set_anti_alias(true);
    // p.setColor(SK_ColorRED);
    p.set_color(Color::new(0xFFFF_0000));
    // canvas.drawCircle(SkIntToScalar(w)/2, SkIntToScalar(h)/2,
    //                   SkIntToScalar(std::min(w, h))*3/8, p);
    canvas.draw_circle(
        (int_to_scalar(w) / 2.0, int_to_scalar(h) / 2.0),
        int_to_scalar(w.min(h)) * 3.0 / 8.0,
        &p,
    );

    // SkRect r;
    // r.setWH(SkIntToScalar(w), SkIntToScalar(h));
    let r = Rect::from_xywh(0.0, 0.0, int_to_scalar(w), int_to_scalar(h));
    // p.setStyle(SkPaint::kStroke_Style);
    p.set_style(Style::Stroke);
    // p.setStrokeWidth(SkIntToScalar(4));
    p.set_stroke_width(int_to_scalar(4));
    // p.setColor(SK_ColorBLUE);
    p.set_color(Color::new(0xFF00_00FF));
    // canvas.drawRect(r, p);
    canvas.draw_rect(r, &p);
}

/// `class RepeatTileBench`.
// Port of: bench/RepeatTileBench.cpp#L36-L79 (chrome/m156)
struct RepeatTileBench {
    alpha_type: AlphaType,
    paint: Paint,
    name: String,
    bitmap: Bitmap,
}

impl RepeatTileBench {
    // RepeatTileBench(SkColorType ct, SkAlphaType at = kPremul_SkAlphaType)
    // Port of: bench/RepeatTileBench.cpp#L36-L79 (chrome/m156)
    fn new(ct: ColorType, at: AlphaType) -> Self {
        let w = 50;
        let h = 50;

        let mut bitmap = Bitmap::new();
        // fBitmap.setInfo(SkImageInfo::Make(w, h, ct, at));
        // (the C++ ignores the bool result)
        let _ = bitmap.set_info(&ImageInfo::new((w, h), ct, at, None), None);
        // fName.printf("repeatTile_%s_%c", ToolUtils::colortype_name(ct),
        //              kOpaque_SkAlphaType == at ? 'X' : 'A');
        let name = format!(
            "repeatTile_{}_{}",
            colortype_name(ct),
            if at == AlphaType::Opaque { 'X' } else { 'A' }
        );
        Self {
            alpha_type: at,
            paint: Paint::default(),
            name,
            bitmap,
        }
    }
}

impl Benchmark for RepeatTileBench {
    // onGetName()
    fn name(&self) -> String {
        self.name.clone()
    }

    // onDelayedSetup()
    // Port of: bench/RepeatTileBench.cpp#L36-L79 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // fBitmap.allocPixels();
        self.bitmap.alloc_pixels();
        // fBitmap.eraseColor(kOpaque_SkAlphaType == fAlphaType ? SK_ColorWHITE : 0);
        self.bitmap
            .erase_color(Color::new(if self.alpha_type == AlphaType::Opaque {
                0xFFFF_FFFF
            } else {
                0
            }));

        draw_into_bitmap(&mut self.bitmap);

        // fPaint.setShader(fBitmap.makeShader(SkTileMode::kRepeat, SkTileMode::kRepeat,
        //                                     SkSamplingOptions()));
        self.paint.set_shader(self.bitmap.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            None,
        ));
    }

    // onDraw(int loops, SkCanvas* canvas)
    // Port of: bench/RepeatTileBench.cpp#L36-L79 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("RepeatTileBench is a rendering bench");
        // SkPaint paint(fPaint);
        let mut paint = self.paint.clone();
        // this->setupPaint(&paint);
        self.setup_paint(&mut paint);

        // for (int i = 0; i < loops; i++) canvas->drawPaint(paint);
        for _ in 0..loops {
            canvas.draw_paint(&paint);
        }
    }
}

// Port of: bench/RepeatTileBench.cpp#L81-L81 (chrome/m156)
def_bench!(
    repeat_tile_bench_n32_opaque = "RepeatTileBench(kN32_SkColorType, kOpaque_SkAlphaType)",
    RepeatTileBench::new(ColorType::N32, AlphaType::Opaque)
);
// Port of: bench/RepeatTileBench.cpp#L82-L82 (chrome/m156)
def_bench!(
    repeat_tile_bench_n32_premul = "RepeatTileBench(kN32_SkColorType, kPremul_SkAlphaType)",
    RepeatTileBench::new(ColorType::N32, AlphaType::Premul)
);
// Port of: bench/RepeatTileBench.cpp#L83-L83 (chrome/m156)
def_bench!(
    repeat_tile_bench_rgb_565_opaque = "RepeatTileBench(kRGB_565_SkColorType, kOpaque_SkAlphaType)",
    RepeatTileBench::new(ColorType::RGB565, AlphaType::Opaque)
);
