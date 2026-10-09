// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/BlurImageFilterBench.cpp

//! `SkImageFilters::Blur` over a checkerboard bitmap, optionally cropped (`_cropped`) and
//! expanded from a cropped offset input (`_expanded`) (`BlurImageFilterBench`). Rendering bench.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::def_bench;
use crate::prelude::*;

/// `#define FILTER_WIDTH_SMALL 32`, `FILTER_HEIGHT_SMALL 32`.
// Port of: bench/BlurImageFilterBench.cpp#L13-L14 (chrome/m156)
const FILTER_WIDTH_SMALL: i32 = 32;
/// `#define FILTER_HEIGHT_SMALL 32`.
const FILTER_HEIGHT_SMALL: i32 = 32;
/// `#define FILTER_WIDTH_LARGE 256`.
// Port of: bench/BlurImageFilterBench.cpp#L15-L16 (chrome/m156)
const FILTER_WIDTH_LARGE: i32 = 256;
/// `#define FILTER_HEIGHT_LARGE 256`.
const FILTER_HEIGHT_LARGE: i32 = 256;
/// `#define BLUR_SIGMA_MINI 0.5f`.
// Port of: bench/BlurImageFilterBench.cpp#L17-L20 (chrome/m156)
const BLUR_SIGMA_MINI: scalar = 0.5;
/// `#define BLUR_SIGMA_SMALL 1.0f`.
const BLUR_SIGMA_SMALL: scalar = 1.0;
/// `#define BLUR_SIGMA_LARGE 10.0f`.
const BLUR_SIGMA_LARGE: scalar = 10.0;
/// `#define BLUR_SIGMA_HUGE 80.0f`.
const BLUR_SIGMA_HUGE: scalar = 80.0;

/// `make_checkerboard(width, height)`: 16-pixel tiles of two 8×8 squares in two colours, drawn
/// into an N32 bitmap.
// Port of: bench/BlurImageFilterBench.cpp#L24-L47 (chrome/m156)
fn make_checkerboard(width: i32, height: i32) -> Image {
    let mut bm = Bitmap::new();
    // bm.allocN32Pixels(width, height);
    bm.alloc_n32_pixels((width, height), None);
    {
        // SkCanvas canvas(bm);
        let canvas = Canvas::from_bitmap(&mut bm, None).expect("bitmap ready to draw");
        canvas.clear(Color::new(0x0000_0000));
        // SkPaint darkPaint; darkPaint.setColor(0xFF804020);
        let mut dark_paint = Paint::default();
        dark_paint.set_color(Color::new(0xFF80_4020));
        // SkPaint lightPaint; lightPaint.setColor(0xFF244484);
        let mut light_paint = Paint::default();
        light_paint.set_color(Color::new(0xFF24_4484));
        for y in (0..height).step_by(16) {
            for x in (0..width).step_by(16) {
                canvas.save();
                canvas.translate((int_to_scalar(x), int_to_scalar(y)));
                canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 8.0, 8.0), &dark_paint);
                canvas.draw_rect(Rect::from_xywh(8.0, 0.0, 8.0, 8.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(0.0, 8.0, 8.0, 8.0), &light_paint);
                canvas.draw_rect(Rect::from_xywh(8.0, 8.0, 8.0, 8.0), &dark_paint);
                canvas.restore();
            }
        }
    }
    // return bm.asImage();
    bm.as_image().expect("checkerboard bitmap has pixels")
}

/// `class BlurImageFilterBench`.
// Port of: bench/BlurImageFilterBench.cpp#L49-L121 (chrome/m156)
struct BlurImageFilterBench {
    name: String,
    is_small: bool,
    is_cropped: bool,
    is_expanded: bool,
    sigma_x: scalar,
    sigma_y: scalar,
    checkerboard: Option<Image>,
}

impl BlurImageFilterBench {
    // Port of: bench/BlurImageFilterBench.cpp#L51-L64 (chrome/m156)
    fn new(
        sigma_x: scalar,
        sigma_y: scalar,
        small: bool,
        cropped: bool,
        expanded: bool,
    ) -> Self {
        // SkASSERT(!fIsExpanded || fIsCropped);
        assert!(!expanded || cropped, "never want expansion without cropping");
        Self {
            // fName.printf("blur_image_filter_%s%s%s_%.2f_%.2f", ...)
            name: format!(
                "blur_image_filter_{}{}{}_{sigma_x:.2}_{sigma_y:.2}",
                if small { "small" } else { "large" },
                if cropped { "_cropped" } else { "" },
                if expanded { "_expanded" } else { "" },
            ),
            is_small: small,
            is_cropped: cropped,
            is_expanded: expanded,
            sigma_x,
            sigma_y,
            checkerboard: None,
        }
    }
}

impl Benchmark for BlurImageFilterBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/BlurImageFilterBench.cpp#L70-L77 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        if self.checkerboard.is_none() {
            self.checkerboard = Some(make_checkerboard(
                if self.is_small {
                    FILTER_WIDTH_SMALL
                } else {
                    FILTER_WIDTH_LARGE
                },
                if self.is_small {
                    FILTER_HEIGHT_SMALL
                } else {
                    FILTER_HEIGHT_LARGE
                },
            ));
        }
    }

    // Port of: bench/BlurImageFilterBench.cpp#L79-L107 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("BlurImageFilterBench is a rendering bench");
        let checkerboard = self
            .checkerboard
            .as_ref()
            .expect("delayed setup made the checkerboard");
        // static const int kX = 0; static const int kY = 0;
        let (k_x, k_y) = (0, 0);
        // const SkIRect bmpRect = SkIRect::MakeXYWH(kX, kY, width(), height());
        let bmp_rect = IRect::from_xywh(k_x, k_y, checkerboard.width(), checkerboard.height());
        // const SkIRect bmpRectInset = bmpRect.makeInset(10, 10);
        let mut bmp_rect_inset = bmp_rect;
        bmp_rect_inset.inset((10, 10));
        // sk_sp<SkImageFilter> input = fIsExpanded ? SkImageFilters::Offset(0, 0, nullptr,
        //                                                                  &bmpRectInset) : nullptr;
        let input: Option<ImageFilter> = if self.is_expanded {
            image_filters::offset((0.0, 0.0), None, Some(Rect::from(bmp_rect_inset)))
        } else {
            None
        };
        // const SkIRect* crop = fIsExpanded ? &bmpRect : fIsCropped ? &bmpRectInset : nullptr;
        let crop: Option<Rect> = if self.is_expanded {
            Some(Rect::from(bmp_rect))
        } else if self.is_cropped {
            Some(Rect::from(bmp_rect_inset))
        } else {
            None
        };
        let mut paint = Paint::default();
        // paint.setImageFilter(SkImageFilters::Blur(fSigmaX, fSigmaY, std::move(input), crop));
        // The overload without a tile mode defaults to the decal tile mode.
        paint.set_image_filter(image_filters::blur(
            self.sigma_x,
            self.sigma_y,
            TileMode::Decal,
            input,
            crop,
        ));
        // SkSamplingOptions sampling;
        let sampling = SamplingOptions::default();
        for _ in 0..loops {
            // canvas->drawImage(fCheckerboard, kX, kY, sampling, &paint);
            canvas.draw_image_with_sampling_options(
                checkerboard,
                (int_to_scalar(k_x), int_to_scalar(k_y)),
                sampling,
                Some(&paint),
            );
        }
    }
}

// Port of: bench/BlurImageFilterBench.cpp#L126-L126 (chrome/m156)
def_bench!(
    blur_image_filter_bench_01 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, 0, false, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, 0.0, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L127-L127 (chrome/m156)
def_bench!(
    blur_image_filter_bench_02 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, 0, false, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, 0.0, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L128-L128 (chrome/m156)
def_bench!(
    blur_image_filter_bench_03 = "BlurImageFilterBench(0, BLUR_SIGMA_LARGE, false, false, false)",
    BlurImageFilterBench::new(0.0, BLUR_SIGMA_LARGE, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L129-L129 (chrome/m156)
def_bench!(
    blur_image_filter_bench_04 = "BlurImageFilterBench(0, BLUR_SIGMA_SMALL, false, false, false)",
    BlurImageFilterBench::new(0.0, BLUR_SIGMA_SMALL, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L130-L130 (chrome/m156)
def_bench!(
    blur_image_filter_bench_05 = "BlurImageFilterBench(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, true, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, true, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L131-L131 (chrome/m156)
def_bench!(
    blur_image_filter_bench_06 = "BlurImageFilterBench(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, false, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L132-L132 (chrome/m156)
def_bench!(
    blur_image_filter_bench_07 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, true, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, true, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L133-L133 (chrome/m156)
def_bench!(
    blur_image_filter_bench_08 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, false, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L134-L134 (chrome/m156)
def_bench!(
    blur_image_filter_bench_09 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, true, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, true, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L135-L135 (chrome/m156)
def_bench!(
    blur_image_filter_bench_10 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, false, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L136-L136 (chrome/m156)
def_bench!(
    blur_image_filter_bench_11 = "BlurImageFilterBench(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, true, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, true, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L137-L137 (chrome/m156)
def_bench!(
    blur_image_filter_bench_12 = "BlurImageFilterBench(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, false, false, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, false, false, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L139-L139 (chrome/m156)
def_bench!(
    blur_image_filter_bench_13 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, 0, false, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, 0.0, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L140-L140 (chrome/m156)
def_bench!(
    blur_image_filter_bench_14 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, 0, false, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, 0.0, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L141-L141 (chrome/m156)
def_bench!(
    blur_image_filter_bench_15 = "BlurImageFilterBench(0, BLUR_SIGMA_LARGE, false, true, false)",
    BlurImageFilterBench::new(0.0, BLUR_SIGMA_LARGE, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L142-L142 (chrome/m156)
def_bench!(
    blur_image_filter_bench_16 = "BlurImageFilterBench(0, BLUR_SIGMA_SMALL, false, true, false)",
    BlurImageFilterBench::new(0.0, BLUR_SIGMA_SMALL, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L143-L143 (chrome/m156)
def_bench!(
    blur_image_filter_bench_17 = "BlurImageFilterBench(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, true, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, true, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L144-L144 (chrome/m156)
def_bench!(
    blur_image_filter_bench_18 = "BlurImageFilterBench(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, false, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L145-L145 (chrome/m156)
def_bench!(
    blur_image_filter_bench_19 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, true, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, true, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L146-L146 (chrome/m156)
def_bench!(
    blur_image_filter_bench_20 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, false, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L147-L147 (chrome/m156)
def_bench!(
    blur_image_filter_bench_21 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, true, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, true, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L148-L148 (chrome/m156)
def_bench!(
    blur_image_filter_bench_22 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, false, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L149-L149 (chrome/m156)
def_bench!(
    blur_image_filter_bench_23 = "BlurImageFilterBench(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, true, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, true, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L150-L150 (chrome/m156)
def_bench!(
    blur_image_filter_bench_24 = "BlurImageFilterBench(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, false, true, false)",
    BlurImageFilterBench::new(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, false, true, false)
);
// Port of: bench/BlurImageFilterBench.cpp#L152-L152 (chrome/m156)
def_bench!(
    blur_image_filter_bench_25 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, 0, false, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, 0.0, false, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L153-L153 (chrome/m156)
def_bench!(
    blur_image_filter_bench_26 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, 0, false, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, 0.0, false, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L154-L154 (chrome/m156)
def_bench!(
    blur_image_filter_bench_27 = "BlurImageFilterBench(0, BLUR_SIGMA_LARGE, false, true, true)",
    BlurImageFilterBench::new(0.0, BLUR_SIGMA_LARGE, false, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L155-L155 (chrome/m156)
def_bench!(
    blur_image_filter_bench_28 = "BlurImageFilterBench(0, BLUR_SIGMA_SMALL, false, true, true)",
    BlurImageFilterBench::new(0.0, BLUR_SIGMA_SMALL, false, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L156-L156 (chrome/m156)
def_bench!(
    blur_image_filter_bench_29 = "BlurImageFilterBench(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, true, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, true, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L157-L157 (chrome/m156)
def_bench!(
    blur_image_filter_bench_30 = "BlurImageFilterBench(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, false, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_MINI, BLUR_SIGMA_MINI, false, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L158-L158 (chrome/m156)
def_bench!(
    blur_image_filter_bench_31 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, true, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, true, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L159-L159 (chrome/m156)
def_bench!(
    blur_image_filter_bench_32 = "BlurImageFilterBench(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, false, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_SMALL, BLUR_SIGMA_SMALL, false, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L160-L160 (chrome/m156)
def_bench!(
    blur_image_filter_bench_33 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, true, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, true, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L161-L161 (chrome/m156)
def_bench!(
    blur_image_filter_bench_34 = "BlurImageFilterBench(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, false, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_LARGE, BLUR_SIGMA_LARGE, false, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L162-L162 (chrome/m156)
def_bench!(
    blur_image_filter_bench_35 = "BlurImageFilterBench(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, true, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, true, true, true)
);
// Port of: bench/BlurImageFilterBench.cpp#L163-L163 (chrome/m156)
def_bench!(
    blur_image_filter_bench_36 = "BlurImageFilterBench(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, false, true, true)",
    BlurImageFilterBench::new(BLUR_SIGMA_HUGE, BLUR_SIGMA_HUGE, false, true, true)
);
