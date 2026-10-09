// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ImageFilterDAGBench.cpp

//! `ImageFilterDAGBench` and `ImageFilterXfermodeIn`: image filter graphs drawn as one rect each
//! (rendering benches). `ImageMakeWithFilterDAGBench` (it reads `images/mandrill_512.png`) and
//! `ImageFilterDisplacedBlur` (no `SkImageFilters::DisplacementMap` in `skia-rust-effects`) are
//! not registered; see their manifest reasons.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters;

use crate::def_bench;
use crate::prelude::*;

/// `class ImageFilterDAGBench`: a blur connected to five inputs of one merge filter.
// Port of: bench/ImageFilterDAGBench.cpp#L27-L64 (chrome/m156)
#[derive(Default)]
struct ImageFilterDagBench;

/// `static const int kNumInputs = 5;` (of `ImageFilterDAGBench`).
// Port of: bench/ImageFilterDAGBench.cpp#L61-L61 (chrome/m156)
const NUM_INPUTS: usize = 5;

impl Benchmark for ImageFilterDagBench {
    // onGetName(): "image_filter_dag".
    fn name(&self) -> String {
        "image_filter_dag".to_owned()
    }

    // onDraw(int loops, SkCanvas* canvas)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ImageFilterDAGBench is a rendering bench");
        // const SkRect rect = SkRect::Make(SkIRect::MakeWH(400, 400));
        let rect = Rect::from_xywh(0.0, 0.0, 400.0, 400.0);
        // Set up the filters once, we're not interested in measuring allocation time here.
        // sk_sp<SkImageFilter> blur(SkImageFilters::Blur(20.0f, 20.0f, nullptr));
        let blur: Option<ImageFilter> =
            image_filters::blur(20.0, 20.0, TileMode::Decal, None, None);
        // sk_sp<SkImageFilter> inputs[kNumInputs]; inputs[i] = blur;
        let inputs: [Option<ImageFilter>; NUM_INPUTS] = std::array::from_fn(|_| blur.clone());
        // SkPaint paint;
        let mut paint = Paint::default();
        // paint.setImageFilter(SkImageFilters::Merge(inputs, kNumInputs));
        paint.set_image_filter(image_filters::merge(&inputs, None));

        // Only measure the filter computations done in drawRect().
        // for (int j = 0; j < loops; j++) canvas->drawRect(rect, paint);
        for _ in 0..loops {
            canvas.draw_rect(rect, &paint);
        }
    }
}

/// `class ImageFilterXfermodeIn`: a `kSrcIn` blend of two offsets of one blur.
// Port of: bench/ImageFilterDAGBench.cpp#L155-L181 (chrome/m156)
#[derive(Default)]
struct ImageFilterXfermodeIn;

impl Benchmark for ImageFilterXfermodeIn {
    // onGetName(): "image_filter_xfermode_in".
    fn name(&self) -> String {
        "image_filter_xfermode_in".to_owned()
    }

    // onDraw(int loops, SkCanvas* canvas)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ImageFilterXfermodeIn is a rendering bench");
        // Allocate filters once to avoid measuring instantiation time.
        // auto blur = SkImageFilters::Blur(20.0f, 20.0f, nullptr);
        let blur = image_filters::blur(20.0, 20.0, TileMode::Decal, None, None);
        // auto offset1 = SkImageFilters::Offset(100.0f, 100.0f, blur);
        let offset1 = image_filters::offset((100.0, 100.0), blur.clone(), None);
        // auto offset2 = SkImageFilters::Offset(-100.0f, -100.0f, blur);
        let offset2 = image_filters::offset((-100.0, -100.0), blur, None);
        // auto xfermode = SkImageFilters::Blend(SkBlendMode::kSrcIn, offset1, offset2, nullptr);
        let xfermode = image_filters::blend(BlendMode::SrcIn, offset1, offset2, None);
        // SkPaint paint;
        // paint.setImageFilter(xfermode);
        let mut paint = Paint::default();
        paint.set_image_filter(xfermode);

        // Measure only the filter time.
        // for (int j = 0; j < loops; j++) canvas->drawRect(SkRect::MakeWH(200.0f, 200.0f), paint);
        for _ in 0..loops {
            canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 200.0, 200.0), &paint);
        }
    }
}

// Port of: bench/ImageFilterDAGBench.cpp#L183-L183 (chrome/m156)
def_bench!(
    image_filter_dag_bench = "ImageFilterDAGBench",
    ImageFilterDagBench
);
// Port of: bench/ImageFilterDAGBench.cpp#L186-L186 (chrome/m156)
def_bench!(
    image_filter_xfermode_in_bench = "ImageFilterXfermodeIn",
    ImageFilterXfermodeIn
);
